use crate::parser::{
    chunked_body_parser::BodyParser,
    first_line_parser::{
        ExtractFirstLine, FirstLineParseError, FirstLineParser, RequestLine, ResponseLine
    },
    front_from_body_parser::parse_front,
    header_parser::{HeaderParseError, HeaderParser},
    http_message_parser::{ParseError, ParsingState},
};
use std::{
    collections::HashMap,
    io::{Read, Result as IoResult, Write},
    net::TcpStream,
};

pub trait ProxyHeadersSender<T> {
    fn send_first_line_and_headers(
        &self,
        remote_host_stream: &mut TcpStream,
        first_line: T,
        headers: &HashMap<String, String>,
    ) -> IoResult<()>;
}
pub struct RequestPartProxySender {
    remote_host_name: &'static str,
}
impl RequestPartProxySender{
    pub fn new(remote_host_name:&'static str)->Self{
        Self { remote_host_name}
    }
}
impl ProxyHeadersSender<RequestLine> for RequestPartProxySender {
    fn send_first_line_and_headers(
        &self,
        remote_host_stream: &mut TcpStream,
        request_line: RequestLine,
        headers: &HashMap<String, String>,
    ) -> IoResult<()> {
        write_proxied_request_line(remote_host_stream, request_line, self.remote_host_name)?;
        write_proxied_headers(remote_host_stream, headers)
    }
}

pub struct ResponsePartProxySender {
    
}

impl ProxyHeadersSender<ResponseLine> for ResponsePartProxySender {
    fn send_first_line_and_headers(
        &self,
        remote_host_stream: &mut TcpStream,
        first_line: ResponseLine,
        headers: &HashMap<String, String>,
    ) -> IoResult<()> {
        write_proxied_response_status_line(remote_host_stream, first_line)?;
        write_proxied_headers(remote_host_stream, headers)
    }
}


pub struct ProxyParser<'a, P: FirstLineParser + ExtractFirstLine, S: ProxyHeadersSender<P::HttpType>> {
    first_line_parser: P,
    header_parser: HeaderParser,
    body_parser: BodyParser,
    body_cursor: usize,
    current_position: usize,
    data: Vec<u8>,
    parsing_state: ParsingState,
    remote_host_stream: &'a mut TcpStream,
    proxy_headers_sender: S,
}
impl<'a, P: FirstLineParser + ExtractFirstLine, S: ProxyHeadersSender<P::HttpType>>
    ProxyParser<'a, P, S>
{
    pub fn new<'b>(
        first_line_parser: P,
        header_parser: HeaderParser,
        body_parser: BodyParser,
        remote_host_stream: &'b mut TcpStream,
        proxy_headers_sender: S,
    ) -> ProxyParser<'b, P, S> {
        ProxyParser {
            first_line_parser,
            header_parser,
            body_parser,
            body_cursor: 0,
            current_position: 0,
            data: Vec::with_capacity(1024),
            parsing_state: ParsingState::FrontSeparateBody,
            remote_host_stream,
            proxy_headers_sender,
        }
    }
    pub fn parse<C: Write + Read>(&mut self, stream: &mut C) -> Result<(), String> {
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
                                                    self.write_first_line_and_headers().map_err(
                                                        |_| "failed to write to ohter proxy 1",
                                                    )?;
                                                    return Ok(());
                                                }
                                            };
                                        if transfer_encoding_chunked == "chunked" {
                                            self.parsing_state = ParsingState::BodyChunked;
                                            self.write_first_line_and_headers()
                                                .map_err(|_| "failed to write to ohter proxy 2")?;
                                            self.remote_host_stream.write_all(&self.data[self.body_cursor..]).map_err(|_| "failed to write to ohter proxy 2")?;
                                        } else {
                                            return Ok(());
                                        }
                                        continue;
                                    }
                                }
                                .parse::<usize>()
                                .map_err(|_| {
                                    "could not parse content length header".to_string()
                                })?;
                                self.parsing_state = ParsingState::BodyContentLength;
                                if self.body_len() >= content_length {
                                    self.write_first_line_and_headers()
                                                .map_err(|_| "failed to write to ohter proxy 2")?;
                                    self.remote_host_stream
                                        .write_all(&self.data[self.body_cursor..])
                                        .map_err(|_| "failed to write to ohter proxy 3")?;
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
                        .header_parser
                        .header("content-length")
                        .ok_or("error occurred")?
                        .parse::<usize>()
                        .map_err(|_| "could not parse content length from header".to_string())?;
                    if self.body_len() >= content_length {
                        self.remote_host_stream
                            .write_all(&self.data[self.body_cursor..])
                            .map_err(|_| "failed to write to other proxy 4")?;
                        return Ok(());
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
                                self.remote_host_stream.write_all(&buf[..n]).map_err(|_| "failed to write to ohter proxy 2")?;
                            }
                            ParseError::HeadersDone => match self.header_parser.header("Trailer") {
                                Some(_) => {
                                    self.parsing_state = ParsingState::TrailerHeaders;
                                }
                                None => {
                                    return Ok(());
                                }
                            },
                            _ => return Ok(()),
                        },
                    }
                }

                ParsingState::BodyDone => {
                    return Ok(());
                }
                ParsingState::TrailerHeaders => match self
                    .header_parser
                    .parse_trailer_header(&self.data[self.current_position..])
                {
                    Ok(offset) => {
                        self.current_position += offset;
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
                            self.remote_host_stream.write_all(&buf[..n]).map_err(|_| "failed to write to ohter proxy 2")?;
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

    fn add_to_data(&mut self, buf: &[u8]) {
        self.data.extend_from_slice(buf);
    }
    fn body_len(&self) -> usize {
        self.data.len() - self.body_cursor
    }
    fn write_first_line_and_headers(&mut self) -> IoResult<()> {
        self.proxy_headers_sender.send_first_line_and_headers(
            self.remote_host_stream,
            self.first_line_parser.get_first_line_ref(),
            self.header_parser.get_headers_ref(),
        )
    }
}

pub fn write_proxied_request_line<T: Write>(
    stream_writer: &mut T,
    request: RequestLine,
    remote_host: &str,
) -> IoResult<()> {
    let status_line = format!(
        "{} {} HTTP/1.1\r\nHost: {}\r\n",
        request.method(),
        request.request_target(),
        remote_host
    );
    stream_writer.write_all(status_line.as_bytes())?;
    Ok(())
}

pub fn write_proxied_headers<T: Write>(
    stream_writer: &mut T,
    headers: &HashMap<String, String>,
) -> IoResult<()> {
    let mut headers_response = String::new();
    for (key, value) in headers {
        if key == "host" {
            continue;
        }
        headers_response.push_str(key.as_str());
        headers_response.push_str(": ");
        headers_response.push_str(value.as_str());
        headers_response.push_str("\r\n");
    }
    headers_response.push_str("\r\n");
    stream_writer.write_all(headers_response.as_bytes())?;
    Ok(())
}


pub fn write_proxied_response_status_line<T: Write>(
    stream_writer: &mut T,
    response: ResponseLine,
) -> IoResult<()> {
    let status_line = format!(
        "HTTP/1.1 {} {}\r\n",
        response.status_code(),
        response.status_message()
    );
    stream_writer.write_all(status_line.as_bytes())?;
    Ok(())
}
