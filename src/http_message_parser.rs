use std::{
    collections::HashMap,
    io::{Cursor, Read},
    net::TcpStream,
};

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
    Done,
}

pub struct NotEnoughBytes;

pub enum HeaderParseError {
    OtherError,
    InvalidHeader(String),
    HeadersDone,
}

pub enum FirstLineParseError {
    FirstLinePartsMissing,
    OtherError,
    MissingHttpVersion,
    InvalidHttpMethod,
}

pub enum GeneralError {
    UnrecoverableError,
    NotEnoughBytes,
}
pub trait HttpMessage {
    type HttpType;
    fn http_message_from_reader(
        &mut self,
        stream: &mut TcpStream,
    ) -> Result<Self::HttpType, String> {
        let mut buf = [0; 1024];
        let mut n = stream
            .read(&mut buf)
            .map_err(|_| "error reading stream".to_string())?;
        self.add_to_data(&buf[..n]);
        loop {
            match self.parsing_state() {
                ParsingState::FrontSeparateBody => {
                    match self.parse_front() {
                        Ok(_) => {
                            self.set_parsing_state(ParsingState::FirstLine)?;
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
                    match self.parse_first_line() {
                        Ok(_) => {
                            self.set_parsing_state(ParsingState::Headers)?;
                        }
                        Err(err) => match err {
                            FirstLineParseError::OtherError => {
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
                                // println!("headers:{:?}",self.headers());
                                let content_length = match self.header("content-length") {
                                    Some(content_len) => {
                                        content_len
                                    }
                                    None => {
                                        let transfer_encoding_chunked =
                                            match self.header("transfer-encoding") {
                                                Some(chunking) => chunking,
                                                None => {
                                                    self.set_parsing_state(ParsingState::BodyContentLength)?;
                                                    
                                                    return Ok(self.create_parsed_http_payload());
                                                }
                                            };
                                        if transfer_encoding_chunked == "chunked" {
                                            self.set_parsing_state(ParsingState::BodyChunked)?;
                                        } else {
                                            self.set_parsing_state(ParsingState::BodyContentLength)?;
                                            return Ok(self.create_parsed_http_payload());
                                        }
                                        continue;
                                    }
                                }
                                .parse::<usize>()
                                .map_err(|_| {
                                    "coluld not parse content length header".to_string()
                                })?;
                                self.set_parsing_state(ParsingState::BodyContentLength)?;
                                if self.body_len() >= content_length {
                                    self.add_to_body()?;
                                    return Ok(self.create_parsed_http_payload());
                                }
                            }
                            HeaderParseError::OtherError => {
                                return Err("another error".into());
                            }
                            HeaderParseError::InvalidHeader(cause) => {
                                return Err(cause);
                            }
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
                        return Ok(self.create_parsed_http_payload());
                    }
                }
                ParsingState::BodyChunked => {
                    if self.body_chunk_part() {
                        match self.add_chunked_body_content() {
                            Ok(_) => {}
                            Err(err) => match err {
                                ParseError::NotEnoughBytes => {
                                    n = stream
                                        .read(&mut buf)
                                        .map_err(|_| "error reading stream".to_string())?;
                                    self.add_to_data(&buf[..n]);
                                }
                                ParseError::OtherError(err) => {
                                    return Err(
                                        err
                                            .to_string(),
                                    );
                                }
                                _ => return Ok(self.create_parsed_http_payload()),
                            },
                        }
                    } else {
                        match self.parse_chunked_body() {
                            Ok(_) => {}
                            Err(err) => match err {
                                ParseError::NotEnoughBytes => {
                                    n = stream
                                        .read(&mut buf)
                                        .map_err(|_| "error reading stream".to_string())?;
                                    self.add_to_data(&buf[..n]);
                                }
                                ParseError::HeadersDone => {
                                    self.set_parsing_state(ParsingState::Done)?;
                                    return Ok(self.create_parsed_http_payload());
                                }
                                _ => return Ok(self.create_parsed_http_payload()),
                            },
                        }
                    }
                }
                ParsingState::Done => {
                    return Ok(self.create_parsed_http_payload());
                }
            }
        }
    }

    fn parse_front(&mut self) -> Result<usize, NotEnoughBytes> {
        let first_index_of_body = find_payload_index(self.data()).ok_or(NotEnoughBytes)?;
        self.set_body_cursor(first_index_of_body);
        Ok(first_index_of_body)
    }
    fn parse_headers(&mut self) -> Result<usize, HeaderParseError> {
        if self.current_position() >= self.body_cursor() - 2 {
            self.set_current_position(2);
            return Err(HeaderParseError::HeadersDone);
        }
        let headers_part = self.current_part();
        let next_field_line_index = find_field_line_index(headers_part).unwrap_or(0);
        let mut cursor = Cursor::new(&headers_part[..next_field_line_index - 2]);
        let mut header_line_str = String::new();
        cursor
            .read_to_string(&mut header_line_str)
            .map_err(|_| HeaderParseError::OtherError)?;
        let parsed_line = parse_headers(&header_line_str)?;
        let (key, value) = parsed_line;
        self.set_current_position(next_field_line_index);
        self.set_headers(key, value);
        Ok(next_field_line_index)
    }
    fn parse_chunked_body(&mut self) -> Result<usize, ParseError> {
        let body = &self.current_part();
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
            usize::from_str_radix(&body_chunk_size_str, 16).map_err(|_| ParseError::OtherError("error in parsing from hexadecimal string".to_string()))?;
        if bytes_to_be_retrieved == 0 {
            return Err(ParseError::HeadersDone);
        }
        self.set_current_position(next_body_data_index);
        self.set_bytes_to_retrieve(bytes_to_be_retrieved);
        self.set_body_chunk_part();

        Ok(next_body_data_index)
    }
    fn add_chunked_body_content(&mut self) -> Result<usize, ParseError> {
        let body = &self.current_part();
        let next_body_data_size_index = match find_field_line_index(body) {
            Some(index) => index,
            None => {
                return Err(ParseError::NotEnoughBytes);
            }
        };

        self.add_chunk_to_body()
            .map_err(|err| ParseError::OtherError(err.to_owned()))?;
        self.set_current_position(next_body_data_size_index);
        self.set_body_chunk_part();
        Ok(2)
    }
    fn parse_first_line(&mut self) -> Result<usize, FirstLineParseError>;
    fn add_to_body(&mut self)->Result<(), &str>;
    fn add_chunk_to_body(&mut self) -> Result<(), &str>;
    fn set_parsing_state(&mut self, parsing_state: ParsingState)->Result<(),&str>;
    fn parsing_state(&self) -> &ParsingState;

    fn create_parsed_http_payload(&self) -> Self::HttpType;
    fn headers(&self) -> &HashMap<String, String>;

    fn set_bytes_to_retrieve(&mut self, bytes_size: usize);
    fn set_body_chunk_part(&mut self);

    fn data(&self) -> &[u8];
    fn free_parsed_data(&mut self);
    fn current_position(&self) -> usize;
    fn set_current_position(&mut self, index: usize);
    fn body_cursor(&self) -> usize;
    fn set_body_cursor(&mut self, index: usize);

    fn set_headers(&mut self, key: String, value: String);
    fn add_to_data(&mut self, buf: &[u8]);
    fn header(&self, key: &str) -> Option<&String>;
    fn body_len(&self) -> usize;
    fn current_part(&self) -> &[u8];
    fn body_chunk_part(&self) -> bool;
}

fn parse_headers(header_field: &str) -> Result<(String, String), HeaderParseError> {
    let broken_parts: Vec<_> = header_field.split(':').collect();

    let key = broken_parts.first().ok_or(HeaderParseError::InvalidHeader(
        "header could not be parsed".to_string(),
    ))?;
    if key.ends_with(' ') {
        return Err(HeaderParseError::InvalidHeader(format!(
            "the key ``{}`` has a space between the field name and colon",
            key
        )));
    }
    if !is_valid_field_name(key) {
        return Err(HeaderParseError::InvalidHeader(format!(
            "the key ``{}`` contains invalid characters",
            key
        )));
    }
    let value = broken_parts[1..].join(":");
    Ok((
        key.to_lowercase().trim().to_string(),
        value.trim().to_string(),
    ))
}
fn find_payload_index(buffer: &[u8]) -> Option<usize> {
    buffer
        .windows(4)
        .position(|w| matches!(w, b"\r\n\r\n"))
        .map(|ix| ix + 4)
}

fn find_field_line_index(buffer: &[u8]) -> Option<usize> {
    buffer
        .windows(2)
        .position(|w| matches!(w, b"\r\n"))
        .map(|ix| ix + 2)
}

fn is_valid_field_name(s: &str) -> bool {
    s.chars().all(|c| {
        c.is_ascii_alphanumeric()
            || matches!(
                c,
                '!' | '#'
                    | '$'
                    | '%'
                    | '&'
                    | '\''
                    | '*'
                    | '+'
                    | '-'
                    | '.'
                    | '^'
                    | '_'
                    | '`'
                    | '|'
                    | '~'
            )
    })
}
