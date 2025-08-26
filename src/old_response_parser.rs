use std::{
    collections::HashMap,
    error::Error,
    io::{ Cursor, Read},
    net::TcpStream,
};

pub fn response_from_reader(stream: &mut TcpStream) -> Result<Response, Box<dyn Error>> {
    let mut buf = [0; 1024];
    let mut my_bytes = Vec::<u8>::with_capacity(120);
    let mut no_of_bytes_parsed = 0;
    let mut response = Response::default();
    let mut response_line_parsed = 0;
    let mut n = stream.read(&mut buf)?;
    my_bytes.append(&mut buf[..n].to_vec());
    loop {
        if response_line_parsed==0{
            match response.parse_front(&my_bytes) {
                Ok(_) => {
                    response_line_parsed = 1;
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
                    ParseError::ResponseLinePartsMissing => {
                        return Err("parts of response line missing and could not be parsed".into());
                    }
                    ParseError::MissingHttpVersion => {
                        return Err("the version of http could not be parsed".into());
                    }
                    ParseError::HeadersDone => {
                        return Err("the version of http could not be parsed".into());
                    }
                },
            };

        }
        else if response_line_parsed == 1 {
            match response.parse(&my_bytes) {
                Ok(no) => {
                    response_line_parsed = 2;
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
                    ParseError::ResponseLinePartsMissing => {
                        return Err("parts of response line missing and could not be parsed".into());
                    }
                    ParseError::MissingHttpVersion => {
                        return Err("the version of http could not be parsed".into());
                    }
                    ParseError::HeadersDone => {
                        return Err("the version of http could not be parsed".into());
                    }
                },
            };
        } else if response_line_parsed == 2 {
            match response.parse_response_headers(&my_bytes[no_of_bytes_parsed..]) {
                Ok(no) => {
                    no_of_bytes_parsed += no;
                }
                Err(err) => match err {
                    ParseError::NotEnoughBytes => {
                        n = stream.read(&mut buf)?;
                        my_bytes.append(&mut buf[..n].to_vec());
                    }
                    ParseError::HeadersDone => {
                        response.add_bytes_to_body(&my_bytes[response.body_cursor..]);
                        response_line_parsed = 3;
                        let content_length = match response.headers.get("content-length") {
                            Some(content_len) => content_len,
                            None => {
                                let transfer_encoding_chunked =
                                    match response.headers.get("transfer-encoding") {
                                        Some(chunking) => chunking,
                                        None => {
                                            return Ok(response);
                                        }
                                    };
                                if transfer_encoding_chunked == "chunked" {
                                    response_line_parsed = 4;
                                } else {
                                    return Ok(response);
                                }
                                continue;
                            }
                        }
                        .parse::<usize>()?;
                        if response.body.len() >= content_length {
                            return Ok(response);
                        }
                    }
                    ParseError::OtherError => {
                        return Err("another error".into());
                    }
                    ParseError::InvalidHeader(cause) => {
                        return Err(cause.into());
                    }
                    ParseError::ResponseLinePartsMissing => {
                        return Err("parts of response missing and could not be parsed".into());
                    }
                    ParseError::MissingHttpVersion => {
                        return Err("the version of http could not be parsed".into());
                    }
                },
            };
        } else if response_line_parsed == 3 {
            n = stream.read(&mut buf)?;
            response.add_bytes_to_body(&buf[..n]);
            let content_length = response
                .headers
                .get("content-length")
                .ok_or("error occurred")?
                .parse::<usize>()?;
            if response.body.len() >= content_length {
                return Ok(response);
            }
        } else if response.data_content_part {
                if response.current_chunk.is_empty() {
                    n = stream.read(&mut buf)?;
                    response.body.extend_from_slice(&buf[..n]);
                }
                match response.add_chunked_body_content() {
                    Ok(_) => {}
                    Err(err) => match err {
                        ParseError::NotEnoughBytes => {
                            n = stream.read(&mut buf)?;
                            response.current_chunk.extend_from_slice(&buf[..n]);
                        }
                        ParseError::HeadersDone => {
                            return Ok(response);
                        }
                        _ => return Ok(response),
                    },
                }
            } else {
                if response.body.is_empty(){
                    n = stream.read(&mut buf)?;
                    response.body.extend_from_slice(&buf[..n]);
                }
                match response.parse_chunked_body() {
                    Ok(_) => {}
                    Err(err) => match err {
                        ParseError::NotEnoughBytes => {
                            n = stream.read(&mut buf)?;
                            response.body.extend_from_slice(&buf[..n]);
                        }
                        ParseError::HeadersDone => {
                            return Ok(response);
                        }
                        _ => return Ok(response),
                    },
                }
            }
        }
    }

#[derive(Debug)]
enum ParseError {
    NotEnoughBytes,
    ResponseLinePartsMissing,
    OtherError,
    MissingHttpVersion,
    InvalidHeader(String),
    HeadersDone,
}

#[derive(Debug, Default)]
pub struct Response {
    response_line: ResponseLine,
    headers: HashMap<String, String>,
    body: Vec<u8>,
    chunked_body: Vec<u8>,
    data_content_part: bool,
    bytes_to_retrieve: usize,
    current_chunk: Vec<u8>,
    current_position:usize,
    body_cursor:usize
}
#[derive(Debug, Default)]
pub struct ResponseLine {
    http_version: String,
    status_code: String,
    status_message: String,
}

impl ResponseLine {
    pub fn http_version(&self) -> &str {
        &self.http_version
    }
    
    pub fn status_code(&self) -> &str {
        &self.status_code
    }
    
    pub fn status_message(&self) -> &str {
        &self.status_message
    }
}
impl Response{
    fn parse_front(&mut self, response_line: &[u8]) -> Result<usize, ParseError> {
        let first_index_of_body = match find_payload_index(response_line) {
            Some(index) => index,
            None => {
                return Err(ParseError::NotEnoughBytes);
            }
        };
        self.body_cursor=first_index_of_body;
        Ok(first_index_of_body)
    }
    fn parse(&mut self, response_line: &[u8]) -> Result<usize, ParseError> {

        if response_line.is_empty() {
            println!("response line empty");
            return Err(ParseError::OtherError);
        }
        let next_field_line_index = find_field_line_index(response_line).unwrap_or(0);
        self.current_position+=next_field_line_index;
        let mut cursor = Cursor::new(&response_line[..next_field_line_index-2]);
        let mut response_line_str = String::new();
        cursor
            .read_to_string(&mut response_line_str)
            .map_err(|_| ParseError::OtherError)?;
        let parsed_line =
            parse_response_line(&response_line_str).map_err(|_| ParseError::OtherError)?;
        self.response_line = parsed_line;
        Ok(next_field_line_index)
    }
    fn parse_response_headers(&mut self, response_line: &[u8]) -> Result<usize, ParseError> {
        if self.current_position>=self.body_cursor-2{
            return Err(ParseError::HeadersDone);
        }
        let next_field_line_index = find_field_line_index(response_line).unwrap_or(0);
        self.current_position+=next_field_line_index;
        let mut cursor = Cursor::new(&response_line[..next_field_line_index-2]);
        let mut response_line_str = String::new();
        cursor
            .read_to_string(&mut response_line_str)
            .map_err(|_| ParseError::OtherError)?;
        let parsed_line =
            parse_headers(&response_line_str)?;
        let (key, value) = parsed_line;
        self.add_header(key, value);
        Ok(next_field_line_index)
    }
    fn parse_chunked_body(&mut self) -> Result<usize, ParseError> {
        let next_body_data_index = match find_field_line_index(&self.body){
            Some(index) => index,
            None => {
                return Err(ParseError::NotEnoughBytes);
            },
        };
        let mut body_chunk_size_str = String::new();
        let mut cursor = Cursor::new(&self.body[..next_body_data_index-2]);

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
        let next_body_data_size_index = match find_field_line_index(&self.current_chunk){
            Some(index) => index,
            None => {
                return Err(ParseError::NotEnoughBytes);
            },
        };
        let mut body_chunk_data_str = String::new();
        let mut cursor = Cursor::new(&self.current_chunk[..next_body_data_size_index-2]);

        cursor
            .read_to_string(&mut body_chunk_data_str)
            .map_err(|_| ParseError::OtherError)?;
        
        self.data_content_part = false;
        self.chunked_body
            .extend_from_slice(&self.current_chunk[..next_body_data_size_index-2]);
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

fn parse_response_line(response_line: &str) -> Result<ResponseLine, ParseError> {
    let broken_string = response_line.split(' ').collect::<Vec<&str>>();
    if broken_string.len() < 3 {
        return Err(ParseError::ResponseLinePartsMissing);
    }
    let mut http_status = String::new();
    http_status.push_str(broken_string[2]);
    let http_version_parts: Vec<_> = broken_string[0].split('/').collect();
    let http_version = match http_version_parts.get(1) {
        Some(version) => version,
        None => {
            return Err(ParseError::MissingHttpVersion);
        }
    };
    Ok(ResponseLine {
        http_version: http_version.to_string(),
        status_message: http_status,
        status_code: broken_string[1].to_string(),
    })
}

pub fn is_valid_field_name(s: &str) -> bool {
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

pub fn find_payload_index(buffer: &[u8]) -> Option<usize> {
    buffer
        .windows(4)
        .position(|w| matches!(w, b"\r\n\r\n"))
        .map(|ix| ix + 4)
}
pub fn find_field_line_index(buffer: &[u8]) -> Option<usize> {
    buffer
        .windows(2)
        .position(|w| matches!(w, b"\r\n"))
        .map(|ix| ix+2)
}
