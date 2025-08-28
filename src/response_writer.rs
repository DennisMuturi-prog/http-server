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
            headers_response.push_str(key);
            headers_response.push_str(": ");
            headers_response.push_str(value);
            headers_response.push_str("\r\n");
        }
        self.connection.write_all(headers_response.as_bytes())?;
        Ok(Body {
            connection: self.connection,
            transfer_encoding_header_written:false
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
            headers_response.push_str(key);
            headers_response.push_str(": ");
            headers_response.push_str(value);
            headers_response.push_str("\r\n");
        }
        self.connection.write_all(headers_response.as_bytes())?;
        Ok(Body {
            connection: self.connection,
            transfer_encoding_header_written:false
        })

    }
    pub fn write_headers_with_trailer_headers(self,custom_headers:HashMap<&str,&str>,trailer_headers_keys:Vec<&str>)->IoResult<ChunkedBodyWithTrailerHeaders<'a>>{
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
            headers_response.push_str(key);
            headers_response.push_str(": ");
            headers_response.push_str(value);
            headers_response.push_str("\r\n");
        }
        let trailer_headers=format!("Trailer: {}\r\nTransfer-Encoding: chunked\r\n\r\n",trailer_headers_keys.join(""));
        headers_response.push_str(&trailer_headers);
        self.connection.write_all(headers_response.as_bytes())?;
        Ok(ChunkedBodyWithTrailerHeaders  {
            connection: self.connection,
        })

    }
}
pub struct Body<'a> {
    connection: &'a mut TcpStream,
    transfer_encoding_header_written:bool
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
        Ok(Response{})
    }
    pub fn write_body_html(self,body:&str)->IoResult<Response>{
        let mut body_bytes=Vec::<u8>::new();
        body_bytes.extend_from_slice(b"Content-Type: text/html\r\n");
        let content_length_header=format!("Content-Length: {}\r\n\r\n",body.len());
        body_bytes.extend_from_slice(content_length_header.as_bytes());
        body_bytes.extend_from_slice(body.as_bytes());
        self.connection.write_all(&body_bytes)?;
        Ok(Response{ })
    }
    pub fn write_empty_body(self)->IoResult<Response>{
        self.connection.write_all(b"Content-Length: 0\r\n\r\n")?;
        Ok(Response{})
    }
    pub fn write_chunk(&mut self,chunk:&[u8],content_type:ContentType)->IoResult<()>{
        if !self.transfer_encoding_header_written{
            self.connection.write_all(content_type.as_bytes())?;
            self.connection.write_all(b"Transfer-Encoding: chunked\r\n\r\n")?;
            self.transfer_encoding_header_written=true;
        }
        let hex_string_upper = format!("{:X}\r\n", chunk.len());
        self.connection.write_all(hex_string_upper.as_bytes())?;
        self.connection.write_all(chunk)?;
        self.connection.write_all(b"\r\n")?;
        Ok(())
    }
    pub fn write_chunked_body_done(&mut self)->IoResult<Response>{
        self.connection.write_all(b"0\r\n\r\n")?;
        Ok(Response {})
    }
   
}


pub struct ChunkedBodyWithTrailerHeaders<'a> {
    connection: &'a mut TcpStream
}

pub enum ContentType{
    ApplicationJson,
    ImageJpeg,
    TextHtml,
    TextPlain
}

impl ContentType {
    fn as_bytes(&self) -> &[u8] {
        match self {
            ContentType::ApplicationJson => b"Content-Type: application/json\r\n",
            ContentType::ImageJpeg => b"Content-Type: image/jpeg\r\n",
            ContentType::TextHtml => b"Content-Type: text/html\r\n",
            ContentType::TextPlain => b"Content-Type: text/plain\r\n",
        }
    }
}
impl<'a> ChunkedBodyWithTrailerHeaders<'a> {
    pub fn write_chunk(&mut self,chunk:&[u8])->IoResult<()>{
        let hex_string_upper = format!("{:X}\r\n", chunk.len());
        self.connection.write_all(hex_string_upper.as_bytes())?;
        self.connection.write_all(chunk)?;
        self.connection.write_all(b"\r\n")?;
        Ok(())
    }
    pub fn write_chunked_body_done(&mut self)->IoResult<()>{
        self.connection.write_all(b"0\r\n\r\n")?;
        Ok(())
    }
    pub fn write_trailer_headers(self,trailer_headers:HashMap<&str,&str>)->IoResult<Response>{
        let mut headers_response = String::new();        
        for (key, value) in trailer_headers {
            headers_response.push_str(key);
            headers_response.push_str(": ");
            headers_response.push_str(value);
            headers_response.push_str("\r\n");
        }
        headers_response.push_str("\r\n");
        self.connection.write_all(headers_response.as_bytes())?;
        Ok( Response {  })

    }


}

pub struct Response{
}
