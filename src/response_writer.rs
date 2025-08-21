use std::{collections::HashMap};

use crate::server::StatusCode;

pub struct ResponseWriter<'a> {
    bytes: &'a mut Vec<u8>,
}

impl<'a> ResponseWriter<'a> {
    pub fn new<'b>(bytes:&'b mut Vec<u8>)->ResponseWriter<'b>{
        ResponseWriter { bytes}
    }
    pub fn write_status_line(self, status_code: StatusCode) -> Headers<'a> {
        let status = match status_code {
            StatusCode::Ok => "HTTP/1.1 200 OK\r\n",
            StatusCode::BadRequest => "HTTP/1.1 400 Bad Request\r\n",
            StatusCode::InternalServerError => "HTTP/1.1 500 Internal Server Error\r\n",
        };
        self.bytes.extend_from_slice(status.as_bytes());
        Headers {
            bytes:self.bytes
        }
    }
}

pub struct Headers<'a> {
    bytes: &'a mut Vec<u8>,
}
impl<'a> Headers<'a> {
    pub fn write_headers(self, headers: HashMap<String, String>) -> Body<'a>{
        let mut headers_response = String::new();
        for (key, value) in headers {
            headers_response.push_str(&key);
            headers_response.push_str(": ");
            headers_response.push_str(&value);
            headers_response.push_str("\r\n");
        }
        self.bytes.extend_from_slice(headers_response.as_bytes());
        Body {
            bytes: self.bytes,
        }
    }
}
pub struct Body<'a> {
    bytes: &'a mut Vec<u8>,
}

impl<'a> Body<'a> {
    pub fn write_body_plain_text(self,body:&str)->Response {
        self.bytes.extend_from_slice("Content-Type: text/plain\r\n".as_bytes());
        let content_length_header=format!("Content-Length: {}\r\n\r\n",body.len());
        self.bytes.extend_from_slice(content_length_header.as_bytes());
        self.bytes.extend_from_slice(body.as_bytes());
        Response{ anything:0 }
    }
    pub fn write_body_html(self,body:&str)->Response{
        self.bytes.extend_from_slice("Content-Type: text/html\r\n".as_bytes());
        let content_length_header=format!("Content-Length: {}\r\n\r\n",body.len());
        self.bytes.extend_from_slice(content_length_header.as_bytes());
        self.bytes.extend_from_slice(body.as_bytes());
        Response{ anything:0 }
    }
    pub fn write_empty_body(self)->Response{
        self.bytes.extend_from_slice("Content-Type: text/plain\r\n".as_bytes());
        self.bytes.extend_from_slice("Content-Length: 0\r\n\r\n".as_bytes());
        Response{ anything:0 }
    }
}

pub struct Response{
    anything:u8
}
