use std::{
    collections::HashMap,
    io::{ Read, Write},
};
use crate::{parser::{
    chunked_body_parser::BodyParser,
    first_line_parser::{
        FirstLineParseError, FirstLineParser,
        RequestLine, ResponseLine,
    },
    front_from_body_parser::parse_front,
    header_parser::{ HeaderParseError, HeaderParser},
}, routing::HttpVerb};

pub enum ParseError {
    NotEnoughBytes,
    ResponseLinePartsMissing,
    OtherError(String),
    MissingHttpVersion,
    InvalidHeader(String),
    HeadersDone,
}
pub enum ParsingState {
    FrontSeparateBody,
    FirstLine,
    Headers,
    BodyContentLength,
    BodyChunked,
    TrailerHeaders,
    TrailerHeadersDone,
    BodyDone,
    ParsingDone,
}

pub struct Parser<P: FirstLineParser> {
    first_line_parser: P,
    header_parser: HeaderParser,
    body_parser: BodyParser,
    body_cursor: usize,
    current_position: usize,
    data: Vec<u8>,
    parsing_state: ParsingState,
}
impl<P: FirstLineParser> Parser<P> {
    pub fn new(
        first_line_parser: P,
    ) -> Parser<P> {
        Parser {
            first_line_parser,
            header_parser:HeaderParser::default(),
            body_parser:BodyParser::default(),
            body_cursor: 0,
            current_position: 0,
            data: Vec::with_capacity(1024),
            parsing_state: ParsingState::FrontSeparateBody,
        }
    }
    pub fn parse<S: Write + Read>(mut self, stream: &mut S) -> Result<Payload<P::HttpType> , String> {
        let mut buf = [0; 1024];
        let mut n = stream.read(&mut buf).map_err(|err| {
            println!("error in reading {}", err);
            "error reading stream".to_string()
        })?;
        if n == 0 {
            return Err("false alarm".to_string());
        }

        self.add_to_data(&buf[..n]);
        loop {
            match self.parsing_state {
                ParsingState::FrontSeparateBody => {
                    match parse_front(&self.data) {
                        Ok(body_cursor) => {
                            self.body_cursor = body_cursor;
                            self.parsing_state = ParsingState::FirstLine;
                        }
                        Err(_) => {
                            n = stream
                                .read(&mut buf)
                                .map_err(|_| "error reading stream".to_string())?;
                            self.add_to_data(&buf[..n]);
                        }
                    };
                }
                ParsingState::FirstLine => {
                    match self
                        .first_line_parser
                        .parse_first_line(&self.data[self.current_position..])
                    {
                        Ok(next_index) => {
                            self.current_position += next_index;
                            self.parsing_state = ParsingState::Headers;
                        }
                        Err(err) => match err {
                            FirstLineParseError::CursorError => {
                                return Err("another error".into());
                            }
                            FirstLineParseError::FirstLinePartsMissing => {
                                return Err(
                                    "parts of response line missing and could not be parsed".into(),
                                );
                            }
                            FirstLineParseError::MissingHttpVersion => {
                                return Err("the version of http could not be parsed".into());
                            }
                            FirstLineParseError::InvalidHttpMethod => {
                                return Err("invalid http method".into());
                            }
                        },
                    };
                }
                ParsingState::Headers => {
                    match self
                        .header_parser
                        .parse_header(&self.data[self.current_position..])
                    {
                        Ok(offset) => {
                            self.current_position += offset;
                        }
                        Err(err) => match err {
                            HeaderParseError::HeadersDone => {
                                self.current_position += 2;
                                let content_length = match self
                                    .header_parser
                                    .header("content-length")
                                {
                                    Some(content_len) => content_len,
                                    None => {
                                        let transfer_encoding_chunked =
                                            match self.header_parser.header("transfer-encoding") {
                                                Some(chunking) => chunking,
                                                None => {
                                                    self.parsing_state =
                                                        ParsingState::BodyContentLength;

                                                    return Ok(self.create_parsed_payload());
                                                }
                                            };
                                        if transfer_encoding_chunked == "chunked" {
                                            self.parsing_state = ParsingState::BodyChunked;
                                        } else {
                                            self.parsing_state = ParsingState::BodyContentLength;
                                            return Ok(self.create_parsed_payload());
                                        }
                                        continue;
                                    }
                                }
                                .parse::<usize>()
                                .map_err(|_| {
                                    "coluld not parse content length header".to_string()
                                })?;
                                self.parsing_state = ParsingState::BodyContentLength;
                                if self.body_len() >= content_length {
                                    self.body_parser.add_to_body(&self.data[self.body_cursor..]);
                                    return Ok(self.create_parsed_payload());
                                }
                            }
                            HeaderParseError::OtherError => {
                                return Err("another error".into());
                            }
                            HeaderParseError::InvalidHeader(cause) => {
                                return Err(cause);
                            }
                            HeaderParseError::NotEnoughBytes => continue,
                        },
                    };
                }
                ParsingState::BodyContentLength => {
                    n = stream
                        .read(&mut buf)
                        .map_err(|_| "error reading stream".to_string())?;
                    self.add_to_data(&buf[..n]);
                    let content_length = self
                        .header_parser
                        .header("content-length")
                        .ok_or("error occurred")?
                        .parse::<usize>()
                        .map_err(|_| "could not parse content length from header".to_string())?;
                    if self.body_len() >= content_length {
                        self.body_parser.add_to_body(&self.data[self.body_cursor..]);
                        return Ok(self.create_parsed_payload());
                    }
                }
                ParsingState::BodyChunked => {
                    match self
                        .body_parser
                        .parse_body(&self.data[self.current_position..])
                    {
                        Ok(offset) => {
                            self.current_position += offset;
                        }
                        Err(err) => match err {
                            ParseError::NotEnoughBytes => {
                                n = stream
                                    .read(&mut buf)
                                    .map_err(|_| "error reading stream".to_string())?;
                                self.add_to_data(&buf[..n]);
                            }
                            ParseError::HeadersDone => {
                                match self.header_parser.header("Trailer") {
                                Some(_) => {
                                    self.parsing_state = ParsingState::BodyDone;
                                    self.parsing_state = ParsingState::TrailerHeaders;
                                }
                                None => {
                                    self.parsing_state = ParsingState::ParsingDone;
                                    return Ok(self.create_parsed_payload());
                                }
                            }

                            },
                            _ => return Ok(self.create_parsed_payload()),
                        },
                    }
                }

                ParsingState::BodyDone => {
                    return Ok(self.create_parsed_payload());
                }
                ParsingState::TrailerHeaders => match self.header_parser.parse_trailer_header(&self.data[self.current_position..]) {
                    Ok(offset) => {
                        self.current_position+=offset;
                    }
                    Err(err) => match err {
                        HeaderParseError::HeadersDone => {
                            self.parsing_state = ParsingState::TrailerHeadersDone;
                        }
                        HeaderParseError::NotEnoughBytes => {
                            n = stream
                                .read(&mut buf)
                                .map_err(|_| "error reading stream".to_string())?;
                            self.add_to_data(&buf[..n]);
                        }
                        _ => {
                            return Err("an error writing to cursor occurred".to_string());
                        }
                    },
                },
                ParsingState::TrailerHeadersDone => {
                    return Ok(self.create_parsed_payload());
                }
                ParsingState::ParsingDone => {
                    return Ok(self.create_parsed_payload());
                }
            }
        }
    }

    fn add_to_data(&mut self, buf: &[u8]) {
        self.data.extend_from_slice(buf);
    }
    fn body_len(&self) -> usize {
        self.data.len() - self.body_cursor
    }
    pub fn create_parsed_payload(self) ->Payload<P::HttpType>  {
        Payload{
            first_line: self.first_line_parser.get_first_line(),
            headers: self.header_parser.get_headers(),
            body: self.body_parser.get_body(),
        }
    }
}


pub struct Payload<T>{
    first_line:T,
    headers:HashMap<String,String>,
    body:Vec<u8>
}
impl Payload<RequestLine>{

}

impl From<Payload<RequestLine>> for Request{
    fn from(value: Payload<RequestLine>) -> Self {
        Self { request_line: value.first_line, headers: value.headers, body: value.body }
    }
}

impl From<Payload<ResponseLine>> for  Response{
    fn from(value: Payload<ResponseLine>) -> Self {
        Self { response_line: value.first_line, headers: value.headers, body: value.body }
    }
}
pub struct Request {
    request_line: RequestLine,
    headers: HashMap<String, String>,
    body: Vec<u8>
}

impl Request {
    pub fn new(request_line: RequestLine, headers: HashMap<String, String>, body: Vec<u8>) -> Self {
        Self {
            request_line,
            headers,
            body,
        }
    }
    pub fn request_method(&self) -> HttpVerb {
        match self.request_line.method(){
            "GET"=>HttpVerb::GET,
            "POST"=>HttpVerb::POST,
            "PUT"=>HttpVerb::PUT,
            "PATCH"=>HttpVerb::PATCH,
            "DELETE"=>HttpVerb::DELETE,
            _=>HttpVerb::OPTIONS
        }
    }
    pub fn request_path(&self) -> &str {
        let val=self.request_line.request_target().split('?').collect::<Vec<&str>>();
        if !val.is_empty(){
            val[0]
        }
        else{
            ""
        }
    }
    pub fn query_params_string(&self) -> &str {

        let val=self.request_line.request_target().split('?').collect::<Vec<&str>>();
        if val.len()==2{
            val[1]

        }else{
            ""
        }
    }

    pub fn header(&self, header: &str) -> Option<&String> {
        self.headers.get(header)
    }

    pub fn headers(&self) -> &HashMap<String, String> {
        &self.headers
    }

    pub fn body(&self) -> &[u8] {
        &self.body
    }
}

pub struct Response {
    response_line: ResponseLine,
    headers: HashMap<String, String>,
    body: Vec<u8>,
}
impl Response {
    pub fn new(
        response_line: ResponseLine,
        headers: HashMap<String, String>,
        body: Vec<u8>,
    ) -> Self {
        Self {
            response_line,
            headers,
            body,
        }
    }
    pub fn body(&self) -> &[u8] {
        &self.body
    }

    pub fn response_line(&self) -> &ResponseLine {
        &self.response_line
    }

    pub fn headers(&self) -> &HashMap<String, String> {
        &self.headers
    }
}

pub fn find_field_line_index(buffer: &[u8]) -> Option<usize> {
    buffer
        .windows(2)
        .position(|w| matches!(w, b"\r\n"))
        .map(|ix| ix + 2)
}

pub fn find_payload_index(buffer: &[u8]) -> Option<usize> {
    buffer
        .windows(4)
        .position(|w| matches!(w, b"\r\n\r\n"))
        .map(|ix| ix + 4)
}
