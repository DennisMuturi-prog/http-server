use std::{
    collections::HashMap,
    io::{Cursor, Read, Write},
    net::TcpStream,
};

use crate::{
    old_response_parser::find_field_line_index,
    http_message_parser::{FirstLineParseError, HttpMessage, ParsingState},
    request_parser::{Request, RequestLine, parse_request_line},
    server::{write_proxied_headers, write_proxied_request_status_line},
};

pub struct ProxyRequestParser<'a> {
    request_line: RequestLine,
    headers: HashMap<String, String>,
    body: Vec<u8>,
    body_chunk_part: bool,
    bytes_to_retrieve: usize,
    body_cursor: usize,
    current_position: usize,
    data: Vec<u8>,
    parsing_state: ParsingState,
    remote_host_name: &'static str,
    remote_host_stream: &'a mut TcpStream,
}
impl<'a> ProxyRequestParser<'a> {
    pub fn new(
        remote_host_stream: &'a mut TcpStream,
        remote_host_name: &'static str,
    ) -> ProxyRequestParser<'a> {
        ProxyRequestParser {
            request_line: RequestLine::default(),
            headers: HashMap::new(),
            body: Vec::new(),
            body_chunk_part: false,
            bytes_to_retrieve: 0,
            body_cursor: 0,
            current_position: 0,
            data: Vec::with_capacity(1024),
            remote_host_stream,
            parsing_state: ParsingState::FrontSeparateBody,
            remote_host_name,
        }
    }
}

impl<'a> HttpMessage for ProxyRequestParser<'a> {
    type HttpType = Request;

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
            parse_request_line(&response_line_str).map_err(|_| FirstLineParseError::OtherError)?;
        self.request_line = parsed_line;
        Ok(next_field_line_index)
    }

    fn create_parsed_http_payload(&self) -> Self::HttpType {
        Request::new(
            self.request_line.clone(),
            self.headers.clone(),
            self.body.clone(),
        )
    }
    fn add_chunk_to_body(&mut self) -> Result<(), &str> {
        let end_index = self.current_position + self.bytes_to_retrieve;
        if end_index <= self.data.len() {
            self.body.extend_from_slice(
                &self.data[self.current_position..self.current_position + self.bytes_to_retrieve],
            );
            let mut hex_string_upper = format!("{:X}", self.bytes_to_retrieve);
            hex_string_upper.push_str("\r\n");
            self.remote_host_stream
                .write_all(hex_string_upper.as_bytes())
                .unwrap();
            self.remote_host_stream
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
    fn set_parsing_state(&mut self, parsing_state: ParsingState) {
        match parsing_state {
            ParsingState::FrontSeparateBody => {}
            ParsingState::FirstLine => {}
            ParsingState::Headers => {}
            ParsingState::BodyContentLength => {
                write_proxied_request_status_line(
                    self.remote_host_stream,
                    &self.request_line,
                    self.remote_host_name,
                )
                .unwrap();
                write_proxied_headers(self.remote_host_stream, &self.headers).unwrap();
            }
            ParsingState::BodyChunked => {
                write_proxied_request_status_line(
                    self.remote_host_stream,
                    &self.request_line,
                    "httpbin.org",
                )
                .unwrap();
                write_proxied_headers(self.remote_host_stream, &self.headers).unwrap();
            }
            ParsingState::Done => {
                self.remote_host_stream.write_all(b"0\r\n\r\n").unwrap();
            }
        };
        self.parsing_state = parsing_state
    }
    fn add_to_body(&mut self) {
        self.remote_host_stream
            .write_all(&self.data[self.body_cursor..])
            .unwrap();
        self.body.extend_from_slice(&self.data[self.body_cursor..]);
    }
    fn set_bytes_to_retrieve(&mut self,bytes_size:usize) {
        self.bytes_to_retrieve=bytes_size;
    }
    
    fn set_body_chunk_part(&mut self) {
        self.body_chunk_part = !self.body_chunk_part;
    }
    
    
    
    
    fn current_part(&self) -> &[u8] {
        &self.data[self.current_position..]
    }
    
    
    
    fn set_current_position(&mut self, index: usize) {
        self.current_position+=index;
    }
    
    fn set_body_cursor(&mut self, index: usize) {
        self.body_cursor+=index;
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
    
    
    
    fn add_to_data(&mut self,buf:&[u8]) {
        self.data.extend_from_slice(buf);
    }
    
    
    
    
    fn body_len(&self)->usize {
        self.data.len()-self.body_cursor
    }
    
    
    fn free_parsed_data(&mut self){
        self.current_position=0;

    }
    
    
    fn headers(&self) -> &HashMap<String, String> {
        &self.headers
    }
    
    fn body_chunk_part(&self) -> bool {
        self.body_chunk_part
    }
    
    fn body_cursor(&self) -> usize {
        self.body_cursor
    }
    
    fn current_position(&self) -> usize {
        self.current_position
    }
    
    fn parsing_state(&self) -> &ParsingState {
        &self.parsing_state
    }
    fn header(&self,key:&str)->Option<&String> {
        self.headers.get(key)
    }

    fn data(&self) -> &[u8] {
        &self.data
    }
}
