use std::{collections::HashMap, io::{Write,Result as IoResult}, net::TcpStream};

use crate::server::{get_common_headers, StatusCode};

pub struct ResponseWriter<'a> {
    connection: &'a mut TcpStream,
}

impl<'a> ResponseWriter<'a> {
    pub fn new<'b>(connection:&'b mut TcpStream)->ResponseWriter<'b>{
        ResponseWriter { connection}
    }
    pub fn write_status_line(self, status_code: StatusCode) -> IoResult<Headers<'a>> {
        let status = match status_code {
            StatusCode::Ok => "HTTP/1.1 200 OK\r\n",
            StatusCode::BadRequest => "HTTP/1.1 400 Bad Request\r\n",
            StatusCode::InternalServerError => "HTTP/1.1 500 Internal Server Error\r\n",
        };
        self.connection.write_all(status.as_bytes())?;
        Ok(Headers {
            connection:self.connection
        })
    }
}

pub struct Headers<'a> {
    connection: &'a mut TcpStream,
}

impl<'a> Headers<'a> {
    pub fn write_default_headers(self) -> IoResult<Body<'a>>{
        let headers=get_common_headers();
        let mut headers_response = String::new();
        for (key, value) in headers {
            headers_response.push_str(&key);
            headers_response.push_str(": ");
            headers_response.push_str(&value);
            headers_response.push_str("\r\n");
        }
        self.connection.write_all(headers_response.as_bytes())?;
        Ok(Body {
            connection: self.connection,
        })
    }
    pub fn write_headers(self,custom_headers:HashMap<&str,&str>)->IoResult<Body<'a>>{
        let mut headers_response = String::new();
        let mut headers=get_common_headers();
        for (key, value) in custom_headers {
            let lower_key=key.to_lowercase();
            if lower_key=="content-type" || lower_key=="content-length" || lower_key=="connection"{
                continue;
            }
            headers.insert(key, value);
        }
        for (key, value) in headers {
            headers_response.push_str(&key);
            headers_response.push_str(": ");
            headers_response.push_str(&value);
            headers_response.push_str("\r\n");
        }
        self.connection.write_all(headers_response.as_bytes())?;
        Ok(Body {
            connection: self.connection,
        })

    }
}
pub struct Body<'a> {
    connection: &'a mut TcpStream
}

impl<'a> Body<'a> {
    pub fn write_body_plain_text(self,body:&str)->IoResult<Response >{
        //add capacity annotation#todo
        let mut body_bytes=Vec::<u8>::new();
        body_bytes.extend_from_slice(b"Content-Type: text/plain\r\n");
        let content_length_header=format!("Content-Length: {}\r\n\r\n",body.len());
        body_bytes.extend_from_slice(content_length_header.as_bytes());
        body_bytes.extend_from_slice(body.as_bytes());
        self.connection.write_all(&body_bytes)?;
        Ok(Response{ anything:0 })
    }
    pub fn write_body_html(self,body:&str)->IoResult<Response>{
        let mut body_bytes=Vec::<u8>::new();
        body_bytes.extend_from_slice(b"Content-Type: text/html\r\n");
        let content_length_header=format!("Content-Length: {}\r\n\r\n",body.len());
        body_bytes.extend_from_slice(content_length_header.as_bytes());
        body_bytes.extend_from_slice(body.as_bytes());
        self.connection.write_all(&body_bytes)?;
        Ok(Response{ anything:0 })
    }
    pub fn write_empty_body(self)->IoResult<Response>{
        self.connection.write_all(b"Content-Length: 0\r\n\r\n")?;
        Ok(Response{ anything:0 })
    }
    pub fn write_chunk(&mut self,chunk:&[u8])->IoResult<()>{
        self.connection.write_all(chunk)?;
        Ok(())
    }
    pub fn write_chunked_body_done(&mut self)->IoResult<Response>{
        self.connection.write_all(b"0\r\n\r\n")?;
        Ok(Response{
            anything:0
        })
    }
}

pub struct Response{
    anything:u8
}
