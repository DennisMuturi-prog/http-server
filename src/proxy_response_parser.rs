use std::{
    collections::HashMap,
    io::{Cursor, Read, Write},
    net::TcpStream,
};

use crate::{
    chunked_parsing::find_field_line_index,
    http_message_parser::{FirstLineParseError, HttpMessage, ParsingState},
    response_parser::{Response, ResponseLine},
    server::{ write_proxied_headers, write_status_line, StatusCode},
};

pub struct ProxyResponseParser<'a> {
    response_line: ResponseLine,
    headers: HashMap<String, String>,
    body: Vec<u8>,
    data_content_part: bool,
    bytes_to_retrieve: usize,
    body_cursor: usize,
    current_position: usize,
    data: Vec<u8>,
    parsing_state: ParsingState,
    client_stream: &'a mut TcpStream,
}
impl<'a> ProxyResponseParser<'a> {
    pub fn new(client_stream: &'a mut TcpStream) -> ProxyResponseParser<'a> {
        ProxyResponseParser {
            response_line: ResponseLine::default(),
            headers: HashMap::new(),
            body: Vec::new(),
            data_content_part: false,
            bytes_to_retrieve: 0,
            body_cursor: 0,
            current_position: 0,
            data: Vec::with_capacity(1024),
            client_stream,
            parsing_state: ParsingState::FrontSeparateBody,
        }
    }
}

impl<'a> HttpMessage for ProxyResponseParser<'a> {
    type HttpType = Response;

    fn parse_first_line(&mut self) -> Result<usize, FirstLineParseError> {
        if self.data.is_empty() {
            println!("response line empty");
            return Err(FirstLineParseError::OtherError);
        }
        let current_part = &&self.data[self.current_position..];
        let next_field_line_index = find_field_line_index(current_part).unwrap_or(0);
        self.current_position += next_field_line_index;
        let mut cursor = Cursor::new(&current_part[..next_field_line_index - 2]);
        let mut response_line_str = String::new();
        cursor
            .read_to_string(&mut response_line_str)
            .map_err(|_| FirstLineParseError::OtherError)?;
        let parsed_line =
            parse_response_line(&response_line_str).map_err(|_| FirstLineParseError::OtherError)?;
        self.response_line = parsed_line;
        Ok(next_field_line_index)
    }

    fn set_bytes_to_retrieve(&mut self, bytes_size: usize) {
        self.bytes_to_retrieve = bytes_size;
    }

    fn set_data_content_part(&mut self) {
        self.data_content_part = !self.data_content_part;
    }

    fn get_data(&self) -> &[u8] {
        &self.data
    }

    fn get_current_part(&self) -> &[u8] {
        &self.data[self.current_position..]
    }

    fn get_current_position(&self) -> usize {
        self.current_position
    }

    fn set_current_position(&mut self, index: usize) {
        self.current_position += index;
    }

    fn get_body_cursor(&self) -> usize {
        self.body_cursor
    }

    fn set_body_cursor(&mut self, index: usize) {
        self.body_cursor += index;
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

    fn add_to_data(&mut self, buf: &[u8]) {
        self.data.extend_from_slice(buf);
    }

    fn get_header(&self, key: &str) -> Option<&String> {
        self.headers.get(key)
    }

    fn get_body_len(&self) -> usize {
        self.data.len() - self.body_cursor
    }

    fn get_data_content_part_state(&self) -> bool {
        self.data_content_part
    }
    fn free_parsed_data(&mut self) {
        self.current_position = 0;
    }

    fn create_parsed_http_payload(&self) -> Self::HttpType {
        Response::new(
            self.response_line.clone(),
            self.headers.clone(),
            self.body.clone(),
        )
    }
    fn add_to_body(&mut self) {
        self.client_stream.write_all(&self.data[self.body_cursor..]).unwrap();
        self.body.extend_from_slice(&self.data[self.body_cursor..]);
    }
    fn add_chunk_to_body(&mut self) -> Result<(), &str> {
        let end_index = self.current_position + self.bytes_to_retrieve;
        if end_index <= self.data.len() {
            self.body.extend_from_slice(
                &self.data[self.current_position..self.current_position + self.bytes_to_retrieve],
            );
            let mut hex_string_upper = format!("{:X}", self.bytes_to_retrieve);
            hex_string_upper.push_str("\r\n");
            self.client_stream
                .write_all(hex_string_upper.as_bytes())
                .unwrap();
            self.client_stream
                .write_all(
                    &self.data
                        [self.current_position..self.current_position + self.bytes_to_retrieve + 2],
                )
                .unwrap();

            Ok(())
        } else {
            Err("wrong transfer chunk encoding")
        }
    }
    fn get_headers(&self) -> HashMap<String, String> {
        self.headers.clone()
    }
    fn set_parsing_state(&mut self, parsing_state: ParsingState) {
        match parsing_state {
            ParsingState::FrontSeparateBody => {}
            ParsingState::FirstLine => {},
            ParsingState::Headers => {}
            ParsingState::BodyContentLength => {
                write_status_line(self.client_stream, StatusCode::Ok).unwrap();
                write_proxied_headers(
                    self.client_stream,
                    &self.headers.clone()
                )
                .unwrap();

            },
            ParsingState::BodyChunked => {
                write_status_line(self.client_stream, StatusCode::Ok).unwrap();
                write_proxied_headers(
                    self.client_stream,
                    &self.headers
                )
                .unwrap();
            }
            ParsingState::Done => {
                self.client_stream.write_all(b"0\r\n\r\n").unwrap();
            }
        };
        self.parsing_state = parsing_state
    }

    fn get_parsing_state(&self) -> &ParsingState {
        &self.parsing_state
    }
}

fn parse_response_line(response_line: &str) -> Result<ResponseLine, FirstLineParseError> {
    let broken_string = response_line.split(' ').collect::<Vec<&str>>();
    if broken_string.len() < 3 {
        return Err(FirstLineParseError::FirstLinePartsMissing);
    }
    let mut http_status_message = String::new();
    http_status_message.push_str(broken_string[2]);
    let http_version_parts: Vec<_> = broken_string[0].split('/').collect();
    let http_version = match http_version_parts.get(1) {
        Some(version) => version,
        None => {
            return Err(FirstLineParseError::MissingHttpVersion);
        }
    };
    Ok(ResponseLine::new(
        http_version.to_string(),
        broken_string[1].to_string(),
        http_status_message,
    ))
}
