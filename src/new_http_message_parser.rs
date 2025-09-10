use std::{
    collections::HashMap,
    io::{Cursor, Read, Write},
};

use crate::{
    http_message_parser::{
        HeaderParseError, NotEnoughBytes, ParseError, ParsingState, parse_headers,
    },
};

pub enum FirstLineParseError {
    FirstLinePartsMissing,
    CursorError,
    MissingHttpVersion,
    InvalidHttpMethod,
}

pub trait FirstLineParser {
    fn parse_first_line(&mut self, data: &[u8]) -> Result<usize, FirstLineParseError>;
}
#[derive(Default)]
pub struct FirstLineRequestParser {
    http_version: String,
    request_target: String,
    method: String,
}
impl FirstLineParser for FirstLineRequestParser {
    fn parse_first_line(&mut self, data: &[u8]) -> Result<usize, FirstLineParseError> {
        let next_field_line_index = find_field_line_index(data).unwrap_or(0);
        let mut cursor = Cursor::new(&data[..next_field_line_index - 2]);
        let mut response_line_str = String::new();
        cursor
            .read_to_string(&mut response_line_str)
            .map_err(|_| FirstLineParseError::CursorError)?;
        let parsed_line = parse_request_line(&response_line_str)?;
        self.set(parsed_line);
        Ok(next_field_line_index)
    }
}
impl FirstLineRequestParser {
    fn set(&mut self, request_line: RequestLine) {
        self.http_version = request_line.http_version;
        self.method = request_line.method;
        self.request_target = request_line.request_target;
    }
    fn get_first_line(self) -> RequestLine{
        RequestLine {
            http_version: self.http_version,
            request_target: self.request_target,
            method: self.method,
        }
    }
}

#[derive(Default)]
struct FirstLineResponseParser {
    http_version: String,
    status_code: String,
    status_message: String,
}

impl FirstLineParser for FirstLineResponseParser {
    fn parse_first_line(&mut self, data: &[u8]) -> Result<usize, FirstLineParseError> {
        let next_field_line_index = find_field_line_index(data).unwrap_or(0);
        let mut cursor = Cursor::new(&data[..next_field_line_index - 2]);
        let mut response_line_str = String::new();
        cursor
            .read_to_string(&mut response_line_str)
            .map_err(|_| FirstLineParseError::CursorError)?;
        let parsed_line = parse_response_line(&response_line_str)?;
        self.set(parsed_line);
        Ok(next_field_line_index)
    }
}
impl FirstLineResponseParser {
    fn set(&mut self, response_line: ResponseLine) {
        self.http_version = response_line.http_version;
        self.status_code = response_line.status_code;
        self.status_message = response_line.status_message;
    }
    fn get_first_line(self) -> ResponseLine {
        ResponseLine {
            http_version: self.http_version,
            status_code: self.status_code,
            status_message: self.status_message,
        }
    }
}
enum BodyChunkPart {
    DataSizePart,
    DataContentPart,
}

pub struct Parser<P: FirstLineParser> {
    first_line_parser: P,
    headers: HashMap<String, String>,
    trailer_headers: HashMap<String, String>,
    body: Vec<u8>,
    body_chunk_part: BodyChunkPart,
    bytes_to_retrieve: usize,
    body_cursor: usize,
    current_position: usize,
    data: Vec<u8>,
    parsing_state: ParsingState,
}
impl<P: FirstLineParser> Parser<P> {
    pub fn new(first_line_parser: P) -> Parser<P> {
        Parser {
            first_line_parser,
            headers: HashMap::new(),
            trailer_headers: HashMap::new(),
            body: Vec::new(),
            body_chunk_part: BodyChunkPart::DataSizePart,
            bytes_to_retrieve: 0,
            body_cursor: 0,
            current_position: 0,
            data: Vec::with_capacity(1024),
            parsing_state: ParsingState::FrontSeparateBody,
        }
    }
    pub fn parse<S: Write + Read>(&mut self, stream: &mut S) -> Result<(), String> {
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
                    match self.parse_front() {
                        Ok(_) => {
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
                    match self.parse_headers() {
                        Ok(_) => {}
                        Err(err) => match err {
                            HeaderParseError::HeadersDone => {
                                let content_length = match self.header("content-length") {
                                    Some(content_len) => content_len,
                                    None => {
                                        let transfer_encoding_chunked =
                                            match self.header("transfer-encoding") {
                                                Some(chunking) => chunking,
                                                None => {
                                                    self.parsing_state =
                                                        ParsingState::BodyContentLength;

                                                    return Ok(());
                                                }
                                            };
                                        if transfer_encoding_chunked == "chunked" {
                                            self.parsing_state = ParsingState::BodyChunked;
                                        } else {
                                            self.parsing_state = ParsingState::BodyContentLength;
                                            return Ok(());
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
                                    self.add_to_body()?;
                                    return Ok(());
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
                        .header("content-length")
                        .ok_or("error occurred")?
                        .parse::<usize>()
                        .map_err(|_| "could not parse content length from header".to_string())?;
                    if self.body_len() >= content_length {
                        self.add_to_body()?;
                        return Ok(());
                    }
                }
                ParsingState::BodyChunked => match self.body_chunk_part {
                    BodyChunkPart::DataSizePart => match self.parse_chunked_body_size() {
                        Ok(_) => {}
                        Err(err) => match err {
                            ParseError::NotEnoughBytes => {
                                n = stream
                                    .read(&mut buf)
                                    .map_err(|_| "error reading stream".to_string())?;
                                self.add_to_data(&buf[..n]);
                            }
                            ParseError::HeadersDone => match self.header("Trailer") {
                                Some(_) => {
                                    self.parsing_state = ParsingState::BodyDone;
                                    self.parsing_state = ParsingState::TrailerHeaders;
                                }
                                None => {
                                    self.parsing_state = ParsingState::ParsingDone;
                                    return Ok(());
                                }
                            },
                            _ => return Ok(()),
                        },
                    },

                    BodyChunkPart::DataContentPart => match self.parse_chunked_body_content() {
                        Ok(_) => {}
                        Err(err) => match err {
                            ParseError::NotEnoughBytes => {
                                n = stream
                                    .read(&mut buf)
                                    .map_err(|_| "error reading stream".to_string())?;
                                self.add_to_data(&buf[..n]);
                            }
                            ParseError::OtherError(err) => {
                                return Err(err.to_string());
                            }
                            _ => return Ok(()),
                        },
                    },
                },
                ParsingState::BodyDone => {
                    return Ok(());
                }
                ParsingState::TrailerHeaders => match self.parse_trailer_headers() {
                    Ok(_) => {}
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
                    return Ok(());
                }
                ParsingState::ParsingDone => {
                    return Ok(());
                }
            }
        }
    }

    fn parse_front(&mut self) -> Result<(), NotEnoughBytes> {
        let first_index_of_body = find_payload_index(&self.data).ok_or(NotEnoughBytes)?;
        self.body_cursor = first_index_of_body;
        Ok(())
    }

    fn parse_headers(&mut self) -> Result<(), HeaderParseError> {
        if self.current_position >= self.body_cursor - 2 {
            self.current_position += 2;
            return Err(HeaderParseError::HeadersDone);
        }
        let headers_part = &self.data[self.current_position..];
        let next_field_line_index = find_field_line_index(headers_part).unwrap_or(0);
        let mut cursor = Cursor::new(&headers_part[..next_field_line_index - 2]);
        let mut header_line_str = String::new();
        cursor
            .read_to_string(&mut header_line_str)
            .map_err(|_| HeaderParseError::OtherError)?;
        let parsed_line = parse_headers(&header_line_str)?;
        let (key, value) = parsed_line;
        self.current_position += next_field_line_index;
        self.set_headers(key, value);
        Ok(())
    }

    fn parse_chunked_body_size(&mut self) -> Result<(), ParseError> {
        let body = &self.data[self.current_position..];
        let next_body_data_index = match find_field_line_index(body) {
            Some(index) => index,
            None => {
                return Err(ParseError::NotEnoughBytes);
            }
        };
        let mut body_chunk_size_str = String::new();
        let mut cursor = Cursor::new(&body[..next_body_data_index - 2]);

        cursor
            .read_to_string(&mut body_chunk_size_str)
            .map_err(|_| ParseError::OtherError("error in reading string to cursor".to_string()))?;
        let bytes_to_be_retrieved =
            usize::from_str_radix(&body_chunk_size_str, 16).map_err(|_| {
                ParseError::OtherError("error in parsing from hexadecimal string".to_string())
            })?;
        if bytes_to_be_retrieved == 0 {
            self.current_position += next_body_data_index + 2;
            return Err(ParseError::HeadersDone);
        }
        self.current_position += next_body_data_index;
        self.bytes_to_retrieve = bytes_to_be_retrieved;
        self.set_body_chunk_part();
        Ok(())
    }

    fn parse_chunked_body_content(&mut self) -> Result<(), ParseError> {
        let body = &self.data[self.current_position..];
        let next_body_data_size_index = match find_field_line_index(body) {
            Some(index) => index,
            None => {
                return Err(ParseError::NotEnoughBytes);
            }
        };

        self.add_chunk_to_body()
            .map_err(|err| ParseError::OtherError(err.to_owned()))?;
        self.current_position += next_body_data_size_index;
        self.set_body_chunk_part();
        Ok(())
    }
    fn parse_trailer_headers(&mut self) -> Result<(), HeaderParseError> {
        let headers_part = &self.data[self.current_position..];
        if headers_part.starts_with(b"\r\n") {
            return Err(HeaderParseError::HeadersDone);
        }
        let next_field_line_index =
            find_field_line_index(headers_part).ok_or(HeaderParseError::NotEnoughBytes)?;
        let mut cursor = Cursor::new(&headers_part[..next_field_line_index - 2]);
        let mut header_line_str = String::new();
        cursor
            .read_to_string(&mut header_line_str)
            .map_err(|_| HeaderParseError::OtherError)?;
        let parsed_line = parse_headers(&header_line_str)?;
        let (key, value) = parsed_line;
        self.current_position += next_field_line_index;
        self.set_trailer_headers(key, value);
        Ok(())
    }
    fn add_to_data(&mut self, buf: &[u8]) {
        self.data.extend_from_slice(buf);
    }

    fn add_chunk_to_body(&mut self) -> Result<(), &str> {
        let end_index = self.current_position + self.bytes_to_retrieve;
        if end_index <= self.data.len() {
            self.body.extend_from_slice(
                &self.data[self.current_position..self.current_position + self.bytes_to_retrieve],
            );
            Ok(())
        } else {
            Err("wrong transfer chunk encoding")
        }
    }
    fn header(&self, key: &str) -> Option<&String> {
        self.headers.get(key)
    }
    fn set_body_chunk_part(&mut self) {
        match self.body_chunk_part {
            BodyChunkPart::DataSizePart => self.body_chunk_part = BodyChunkPart::DataContentPart,
            BodyChunkPart::DataContentPart => self.body_chunk_part = BodyChunkPart::DataSizePart,
        }
    }

    fn set_headers(&mut self, key: String, value: String) {
        self.headers
            .entry(key)
            .and_modify(|existing| {
                existing.push(','); // HTTP header values separated by comma-space
                existing.push_str(&value);
            })
            .or_insert(value);
    }
    fn body_len(&self) -> usize {
        self.data.len() - self.body_cursor
    }
    fn add_to_body(&mut self) -> Result<(), &str> {
        self.body.extend_from_slice(&self.data[self.body_cursor..]);
        Ok(())
    }
    fn set_trailer_headers(&mut self, key: String, value: String) {
        self.trailer_headers
            .entry(key)
            .and_modify(|existing| {
                existing.push(','); // HTTP header values separated by comma-space
                existing.push_str(&value);
            })
            .or_insert(value);
    }
}

impl Parser<FirstLineRequestParser> {
    pub fn create_request_payload(self) -> Request {
        Request {
            request_line: self.first_line_parser.get_first_line(),
            headers: self.headers,
            body: self.body,
        }
    }
}

impl Parser<FirstLineResponseParser>{
    pub fn create_response_payload(self) -> Response {
    Response {
        response_line: self.first_line_parser.get_first_line(),
        headers: self.headers,
        body: self.body,
    }
}

}

pub fn parse_request_line(request_line: &str) -> Result<RequestLine, FirstLineParseError> {
    let http_verbs = ["GET", "POST", "PATCH", "DELETE", "PUT", "OPTIONS"];
    let broken_string = request_line.split(' ').collect::<Vec<&str>>();
    if broken_string.len() < 3 {
        return Err(FirstLineParseError::FirstLinePartsMissing);
    }
    let mut http_verb = String::new();
    if http_verbs.contains(&broken_string[0]) {
        http_verb.push_str(broken_string[0]);
    } else {
        return Err(FirstLineParseError::InvalidHttpMethod);
    }
    let http_version_parts: Vec<_> = broken_string[2].split('/').collect();
    let http_version = match http_version_parts.get(1) {
        Some(version) => version,
        None => {
            return Err(FirstLineParseError::MissingHttpVersion);
        }
    };
    Ok(RequestLine {
        http_version: http_version.to_string(),
        method: http_verb,
        request_target: broken_string[1].to_string(),
    })
}

pub fn parse_response_line(response_line: &str) -> Result<ResponseLine, FirstLineParseError> {
    let broken_string = response_line.split(' ').collect::<Vec<&str>>();
    if broken_string.len() < 3 {
        return Err(FirstLineParseError::FirstLinePartsMissing);
    }
    let mut http_status = String::new();
    http_status.push_str(broken_string[2]);
    let http_version_parts: Vec<_> = broken_string[0].split('/').collect();
    let http_version = match http_version_parts.get(1) {
        Some(version) => version,
        None => {
            return Err(FirstLineParseError::MissingHttpVersion);
        }
    };
    Ok(ResponseLine {
        http_version: http_version.to_string(),
        status_message: http_status,
        status_code: broken_string[1].to_string(),
    })
}



pub struct Request {
    request_line: RequestLine,
    headers: HashMap<String, String>,
    body: Vec<u8>,
}

impl Request {
    pub fn new(request_line: RequestLine, headers: HashMap<String, String>, body: Vec<u8>) -> Self {
        Self {
            request_line,
            headers,
            body,
        }
    }
    pub fn request_method(&self) -> &str {
        &self.request_line.method
    }
    pub fn request_path(&self) -> &str {
        &self.request_line.request_target
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

//todo remove clone later
#[derive(Default,Clone)]
pub struct RequestLine {
    http_version: String,
    request_target: String,
    method: String,
}
impl RequestLine {
    pub fn http_version(&self) -> &str {
        &self.http_version
    }

    pub fn request_target(&self) -> &str {
        &self.request_target
    }

    pub fn method(&self) -> &str {
        &self.method
    }
}


#[derive(Default,Clone)]
pub struct ResponseLine {
    http_version: String,
    status_code: String,
    status_message: String,
}
impl ResponseLine {
    pub fn status_code(&self)->&str{
        &self.status_code
    }
    pub fn status_message(&self)->&str{
        &self.status_message
    }
    
    pub fn http_version(&self) -> &str {
        &self.http_version
    }
    
}

pub fn find_field_line_index(buffer: &[u8]) -> Option<usize> {
    buffer
        .windows(2)
        .position(|w| matches!(w, b"\r\n"))
        .map(|ix| ix+2)
}

pub fn find_payload_index(buffer: &[u8]) -> Option<usize> {
    buffer
        .windows(4)
        .position(|w| matches!(w, b"\r\n\r\n"))
        .map(|ix| ix + 4)
}
