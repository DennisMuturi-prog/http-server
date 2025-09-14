use std::io::{Cursor, Read};

use crate::parser::http_message_parser::find_field_line_index;

pub enum FirstLineParseError {
    FirstLinePartsMissing,
    CursorError,
    MissingHttpVersion,
    InvalidHttpMethod,
}
pub trait FirstLineParser{
    type HttpType;
    fn parse_first_line(&mut self, data: &[u8]) -> Result<usize, FirstLineParseError>;
    fn get_first_line(self) -> Self::HttpType;
    fn get_first_line_ref(& self) -> Self::HttpType;
}
#[derive(Default)]
pub struct FirstLineRequestParser {
    http_version: String,
    request_target: String,
    method: String,
}
impl FirstLineParser for FirstLineRequestParser {
    type HttpType = RequestLine;
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
    fn get_first_line(self) -> Self::HttpType{
        RequestLine {
            http_version: self.http_version,
            request_target: self.request_target,
            method: self.method,
        }
    }
    fn get_first_line_ref(& self) -> Self::HttpType {
        RequestLine {
            http_version: self.http_version.to_string(),
            method: self.method.to_string(),
            request_target: self.request_target.to_string(),
        }
    }
    
}
impl FirstLineRequestParser {
    fn set(&mut self, request_line: RequestLine) {
        self.http_version = request_line.http_version;
        self.method = request_line.method;
        self.request_target = request_line.request_target;
    }
}

#[derive(Default)]
pub struct FirstLineResponseParser {
    http_version: String,
    status_code: String,
    status_message: String,
}

impl FirstLineParser for FirstLineResponseParser {
    type HttpType = ResponseLine;
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
    fn get_first_line(self) -> Self::HttpType {
        ResponseLine {
            http_version: self.http_version,
            status_code: self.status_code,
            status_message: self.status_message,
        }
    }
    fn get_first_line_ref(&self) -> Self::HttpType {
        ResponseLine{
            http_version: self.http_version.to_string(),
            status_code: self.status_code.to_string(),
            status_message: self.status_message.to_string(),
        }
    }
}
impl FirstLineResponseParser {
    fn set(&mut self, response_line: ResponseLine) {
        self.http_version = response_line.http_version;
        self.status_code = response_line.status_code;
        self.status_message = response_line.status_message;
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
pub struct RequestLineRef <'a>{
    http_version:&'a str ,
    request_target: &'a str ,
    method: &'a str ,
}
impl<'a> RequestLineRef<'a> {
    pub fn http_version(&self) -> &str {
        self.http_version
    }

    pub fn request_target(&self) -> &str {
        self.request_target
    }

    pub fn method(&self) -> &str {
        self.method
    }
    
}

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



pub struct ResponseLineRef<'a> {
    http_version: &'a str ,
    status_code: &'a str ,
    status_message: &'a str ,
}

impl<'a> ResponseLineRef<'a> {
    pub fn status_code(&self)->&str{
        self.status_code
    }
    pub fn status_message(&self)->&str{
        self.status_message
    }
    
    pub fn http_version(&self) -> &str {
        self.http_version
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

