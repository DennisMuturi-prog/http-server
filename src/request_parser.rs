use std::{collections::HashMap, io::{Cursor, Read}};

use crate::{old_response_parser::find_field_line_index, http_message_parser::{FirstLineParseError, HttpMessage, ParsingState}};
#[derive(Debug, Default,Clone)]
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


// #[derive(Debug)]
pub struct RequestParser{
    request_line: RequestLine,
    headers: HashMap<String, String>,
    trailer_headers: HashMap<String, String>,
    body: Vec<u8>,
    body_chunk_part: bool,
    bytes_to_retrieve: usize,
    body_cursor:usize,
    current_position:usize,
    data:Vec<u8>,
    parsing_state:ParsingState

}






impl Default for RequestParser{
    fn default()->Self{
        Self { request_line: RequestLine::default(), headers: HashMap::new(),trailer_headers: HashMap::new(), body: Vec::new(), body_chunk_part: false, bytes_to_retrieve: 0, body_cursor: 0, current_position: 0, data: Vec::with_capacity(1024),parsing_state:ParsingState::FrontSeparateBody}
    }
}

#[derive(Debug)]

pub struct Request{
    request_line: RequestLine,
    headers: HashMap<String, String>,
    body: Vec<u8>,
}

impl Request{
    pub fn new(request_line:RequestLine,headers:HashMap<String, String>,body:Vec<u8>)->Self{
        Self { request_line, headers, body }

    }
    pub fn request_method(&self)->&str{
        &self.request_line.method
    }
    pub fn request_path(&self)->&str{
        &self.request_line.request_target
    }
    
    pub fn header(&self,header:&str)->Option<&String>{
        self.headers.get(header)
    }
    
    pub fn headers(&self) -> &HashMap<String, String> {
        &self.headers
    }
    
    pub fn body(&self) -> &[u8] {
        &self.body
    }
}

impl HttpMessage for RequestParser{
    type HttpType = Request;
    
    fn parse_first_line(&mut self)->Result<usize,FirstLineParseError> {
        if self.data.is_empty() {
            println!("response line empty");
            return Err(FirstLineParseError::OtherError);
        }
        let current_part=&&self.data[self.current_position..];
        let next_field_line_index = find_field_line_index(current_part).unwrap_or(0);
        self.current_position+=next_field_line_index;
        let mut cursor = Cursor::new(&current_part[..next_field_line_index-2]);
        let mut response_line_str = String::new();
        cursor
            .read_to_string(&mut response_line_str)
            .map_err(|_| FirstLineParseError::OtherError)?;
        let parsed_line =
        parse_request_line(&response_line_str).map_err(|_| FirstLineParseError::OtherError)?;
        self.request_line = parsed_line;
        Ok(next_field_line_index)
    }
    
    fn create_parsed_http_payload(&self)->Self::HttpType {
        Request{
            request_line: self.request_line.clone(),
            headers: self.headers.clone(),
            body: self.body.clone(),
        }
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

    fn set_trailer_headers(&mut self, key: String, value: String) {
        self.trailer_headers
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
    
    
    fn add_to_body(&mut self)->Result<(),&str> {
        self.body.extend_from_slice(&self.data[self.body_cursor..]);
        Ok(())
    }
    
    fn add_chunk_to_body(&mut self)->Result<(),&str> {
        let end_index=self.current_position+self.bytes_to_retrieve;
        if end_index<=self.data.len(){
            self.body.extend_from_slice(&self.data[self.current_position..self.current_position+self.bytes_to_retrieve]);
            Ok(())
        }else{
            Err("wrong transfer chunk encoding")

        }
    }
    
    
    
    fn set_parsing_state(&mut self,parsing_state:ParsingState)->Result<(),&str> {
        self.parsing_state=parsing_state;
        Ok(())
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