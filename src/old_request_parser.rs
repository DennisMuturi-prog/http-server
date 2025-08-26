use std::{
    collections::HashMap,
    error::Error,
    io::{Cursor, Read},
    net::TcpStream,
};

use crate::old_response_parser::{find_field_line_index, find_payload_index, is_valid_field_name};

pub fn request_from_reader(stream: &mut TcpStream) -> Result<Request, Box<dyn Error>> {
    let mut buf = [0; 1024];
    let mut my_bytes = Vec::<u8>::with_capacity(120);
    let mut no_of_bytes_parsed = 0;
    let mut request = Request::default();
    let mut request_line_parsed = 0;
    let mut n = stream.read(&mut buf)?;
    my_bytes.append(&mut buf[..n].to_vec());
    loop {
        if request_line_parsed == 0 {
            match request.parse_front(&my_bytes) {
                Ok(_) => {
                    request_line_parsed = 1;
                }
                Err(err) => match err {
                    ParseError::NotEnoughBytes => {
                        n = stream.read(&mut buf)?;
                        my_bytes.append(&mut buf[..n].to_vec());
                    }
                    ParseError::OtherError => {
                        return Err("another error".into());
                    }
                    ParseError::InvalidHeader(cause) => {
                        return Err(cause.into());
                    }
                    ParseError::RequestLinePartsMissing => {
                        return Err("parts of request line missing and could not be parsed".into());
                    }
                    ParseError::MissingHttpVersion => {
                        return Err("the version of http could not be parsed".into());
                    }
                    ParseError::HeadersDone => {
                        return Err("the version of http could not be parsed".into());
                    }
                    ParseError::InvalidHttpMethod => {
                        return Err("Invalid http verb".into());

                    },
                },
            };
        } else if request_line_parsed == 1 {
            match request.parse(&my_bytes) {
                Ok(no) => {
                    request_line_parsed = 2;
                    no_of_bytes_parsed += no;
                }
                Err(err) => match err {
                    ParseError::NotEnoughBytes => {
                        n = stream.read(&mut buf)?;
                        my_bytes.append(&mut buf[..n].to_vec());
                    }
                    ParseError::OtherError => {
                        return Err("another error".into());
                    }
                    ParseError::InvalidHeader(cause) => {
                        return Err(cause.into());
                    }
                    ParseError::RequestLinePartsMissing => {
                        return Err("parts of request line missing and could not be parsed".into());
                    }
                    ParseError::MissingHttpVersion => {
                        return Err("the version of http could not be parsed".into());
                    }
                    ParseError::HeadersDone => {
                        return Err("the version of http could not be parsed".into());
                    }
                    ParseError::InvalidHttpMethod => {
                        return Err("Invalid http verb".into());

                    },
                },
            };
        } else if request_line_parsed == 2 {
            match request.parse_request_headers(&my_bytes[no_of_bytes_parsed..]) {
                Ok(no) => {
                    no_of_bytes_parsed += no;
                }
                Err(err) => match err {
                    ParseError::NotEnoughBytes => {
                        n = stream.read(&mut buf)?;
                        my_bytes.append(&mut buf[..n].to_vec());
                    }
                    ParseError::HeadersDone => {
                        println!("Request;{:?}", request);
                        request.add_bytes_to_body(&my_bytes[request.body_cursor..]);
                        request_line_parsed = 3;
                        let content_length = match request.headers.get("content-length") {
                            Some(content_len) => content_len,
                            None => {
                                let transfer_encoding_chunked =
                                    match request.headers.get("transfer-encoding") {
                                        Some(chunking) => chunking,
                                        None => {
                                            return Ok(request);
                                        }
                                    };
                                if transfer_encoding_chunked == "chunked" {
                                    request_line_parsed = 4;
                                } else {
                                    return Ok(request);
                                }
                                continue;
                            }
                        }
                        .parse::<usize>()?;
                        if request.body.len() >= content_length {
                            return Ok(request);
                        }
                    }
                    ParseError::OtherError => {
                        return Err("another error".into());
                    }
                    ParseError::InvalidHeader(cause) => {
                        return Err(cause.into());
                    }
                    ParseError::RequestLinePartsMissing => {
                        return Err("parts of request missing and could not be parsed".into());
                    }
                    ParseError::MissingHttpVersion => {
                        return Err("the version of http could not be parsed".into());
                    }
                    ParseError::InvalidHttpMethod => {
                        return Err("Invalid http verb".into());

                    },
                },
            };
        } else if request_line_parsed == 3 {
            n = stream.read(&mut buf)?;
            request.add_bytes_to_body(&buf[..n]);
            let content_length = request
                .headers
                .get("content-length")
                .ok_or("error occurred")?
                .parse::<usize>()?;
            if request.body.len() >= content_length {
                return Ok(request);
            }
        } else if request.data_content_part {
                if request.current_chunk.is_empty() {
                    n = stream.read(&mut buf)?;
                    request.body.extend_from_slice(&buf[..n]);
                }
                match request.add_chunked_body_content() {
                    Ok(_) => {}
                    Err(err) => match err {
                        ParseError::NotEnoughBytes => {
                            n = stream.read(&mut buf)?;
                            request.current_chunk.extend_from_slice(&buf[..n]);
                        }
                        ParseError::HeadersDone => {
                            return Ok(request);
                        }
                        _ => return Ok(request),
                    },
                }
            } else {
                if request.body.is_empty() {
                    n = stream.read(&mut buf)?;
                    request.body.extend_from_slice(&buf[..n]);
                }
                match request.parse_chunked_body() {
                    Ok(_) => {}
                    Err(err) => match err {
                        ParseError::NotEnoughBytes => {
                            n = stream.read(&mut buf)?;
                            request.body.extend_from_slice(&buf[..n]);
                        }
                        ParseError::HeadersDone => {
                            return Ok(request);
                        }
                        _ => return Ok(request),
                    },
                }
            }
        }
    }


#[derive(Debug)]
enum ParseError {
    NotEnoughBytes,
    RequestLinePartsMissing,
    OtherError,
    MissingHttpVersion,
    InvalidHeader(String),
    HeadersDone,
    InvalidHttpMethod,
}

#[derive(Debug, Default)]
pub struct Request {
    request_line: RequestLine,
    headers: HashMap<String, String>,
    body: Vec<u8>,
    chunked_body: Vec<u8>,
    data_content_part: bool,
    bytes_to_retrieve: usize,
    current_chunk: Vec<u8>,
    current_position: usize,
    body_cursor: usize,
}

impl Request {
    fn parse_front(&mut self, request_line: &[u8]) -> Result<usize, ParseError> {
        let first_index_of_body = match find_payload_index(request_line) {
            Some(index) => index,
            None => {
                return Err(ParseError::NotEnoughBytes);
            }
        };
        self.body_cursor = first_index_of_body;
        Ok(first_index_of_body)
    }
    fn parse(&mut self, request_line: &[u8]) -> Result<usize, ParseError> {
        if request_line.is_empty() {
            println!("request line empty");
            return Err(ParseError::OtherError);
        }
        let next_field_line_index = find_field_line_index(request_line).unwrap_or(0);
        self.current_position += next_field_line_index;
        let mut cursor = Cursor::new(&request_line[..next_field_line_index - 2]);
        let mut request_line_str = String::new();
        cursor
            .read_to_string(&mut request_line_str)
            .map_err(|_| ParseError::OtherError)?;
        let parsed_line =
            parse_request_line(&request_line_str).map_err(|_| ParseError::OtherError)?;
        self.request_line = parsed_line;
        Ok(next_field_line_index)
    }
    fn parse_request_headers(&mut self, request_line: &[u8]) -> Result<usize, ParseError> {
        if self.current_position >= self.body_cursor - 2 {
            return Err(ParseError::HeadersDone);
        }
        let next_field_line_index = find_field_line_index(request_line).unwrap_or(0);
        self.current_position += next_field_line_index;
        let mut cursor = Cursor::new(&request_line[..next_field_line_index - 2]);
        let mut request_line_str = String::new();
        cursor
            .read_to_string(&mut request_line_str)
            .map_err(|_| ParseError::OtherError)?;
        let parsed_line = parse_headers(&request_line_str)?;
        let (key, value) = parsed_line;
        self.add_header(key, value);
        Ok(next_field_line_index)
    }
    fn parse_chunked_body(&mut self) -> Result<usize, ParseError> {
        let next_body_data_index = match find_field_line_index(&self.body) {
            Some(index) => index,
            None => {
                return Err(ParseError::NotEnoughBytes);
            }
        };
        let mut body_chunk_size_str = String::new();
        let mut cursor = Cursor::new(&self.body[..next_body_data_index - 2]);

        cursor
            .read_to_string(&mut body_chunk_size_str)
            .map_err(|_| ParseError::OtherError)?;
        let bytes_to_be_retrieved = usize::from_str_radix(&body_chunk_size_str, 16).unwrap();
        if bytes_to_be_retrieved == 0 {
            return Err(ParseError::HeadersDone);
        }
        self.bytes_to_retrieve = bytes_to_be_retrieved;
        self.data_content_part = true;
        self.current_chunk
            .extend_from_slice(&self.body[next_body_data_index..]);
        self.body.clear();
        Ok(next_body_data_index)
    }
    fn add_chunked_body_content(&mut self) -> Result<usize, ParseError> {
        let next_body_data_size_index = match find_field_line_index(&self.current_chunk) {
            Some(index) => index,
            None => {
                return Err(ParseError::NotEnoughBytes);
            }
        };
        let mut body_chunk_data_str = String::new();
        let mut cursor = Cursor::new(&self.current_chunk[..next_body_data_size_index - 2]);

        cursor
            .read_to_string(&mut body_chunk_data_str)
            .map_err(|_| ParseError::OtherError)?;
        println!("data received:{}", body_chunk_data_str);

        self.data_content_part = false;
        self.chunked_body
            .extend_from_slice(&self.current_chunk[..next_body_data_size_index - 2]);
        self.body
            .extend_from_slice(&self.current_chunk[next_body_data_size_index..]);
        self.current_chunk.clear();
        Ok(2)
    }
    fn add_header(&mut self, key: String, value: String) {
        self.headers
            .entry(key)
            .and_modify(|existing| {
                existing.push(','); // HTTP header values separated by comma-space
                existing.push_str(&value);
            })
            .or_insert(value);
    }
    fn add_bytes_to_body(&mut self, buf: &[u8]) {
        self.body.append(&mut buf.to_vec())
    }
    pub fn get_request_method(&self)->&str{
        &self.request_line.method
    }
    pub fn get_request_path(&self)->&str{
        &self.request_line.request_target
    }
}

fn parse_headers(header_field: &str) -> Result<(String, String), ParseError> {
    let broken_parts: Vec<_> = header_field.split(':').collect();

    let key = broken_parts.first().ok_or(ParseError::InvalidHeader(
        "header could not be parsed".to_string(),
    ))?;
    if key.ends_with(' ') {
        return Err(ParseError::InvalidHeader(format!(
            "the key ``{}`` has a space between the field name and colon",
            key
        )));
    }
    if !is_valid_field_name(key) {
        return Err(ParseError::InvalidHeader(format!(
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

#[derive(Debug, Default)]
pub struct RequestLine {
    http_version: String,
    request_target: String,
    method: String,
}

impl RequestLine {
    pub fn http_version(&self) -> &str {
        &self.http_version
    }
}

fn parse_request_line(request_line: &str) -> Result<RequestLine, ParseError> {
    let http_verbs = ["GET", "POST", "PATCH", "DELETE", "PUT", "OPTIONS"];
    let broken_string = request_line.split(' ').collect::<Vec<&str>>();
    if broken_string.len() < 3 {
        return Err(ParseError::RequestLinePartsMissing);
    }
    let mut http_verb = String::new();
    if http_verbs.contains(&broken_string[0]) {
        http_verb.push_str(broken_string[0]);
    } else {
        return Err(ParseError::InvalidHttpMethod);
    }
    let http_version_parts: Vec<_> = broken_string[2].split('/').collect();
    let http_version = match http_version_parts.get(1) {
        Some(version) => version,
        None => {
            return Err(ParseError::MissingHttpVersion);
        }
    };
    Ok(RequestLine {
        http_version: http_version.to_string(),
        method: http_verb,
        request_target: broken_string[1].to_string(),
    })
}
