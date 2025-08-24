use std::{
    collections::HashMap,
    io::{Result as IoResult, Write},
    net::{SocketAddr, TcpListener, TcpStream},
};

use crate::{
    http_message_parser::HttpMessage,
    request_parser::{Request, RequestParser},
    response_writer::{Response, ResponseWriter},
};

pub struct Server<F> {
    listener: TcpListener,
    handler: F,
}

impl<F> Server<F>
where
    F: Fn(ResponseWriter, Request) -> Response,
{
    pub fn serve(port: u16, handler: F) -> IoResult<Self> {
        let listener = TcpListener::bind(SocketAddr::from(([127, 0, 0, 1], port)))?;
        Ok(Server { listener, handler })
    }
    fn handle(&self, mut connection: TcpStream) -> IoResult<()> {
        let mut request_parser = RequestParser::new();
        match request_parser.http_message_from_reader(&mut connection) {
            Ok(request) => {
                if request.get_request_method() == "OPTIONS" {
                    write_status_line(&mut connection, StatusCode::Ok)?;
                    let headers = get_preflight_headers();
                    write_headers(&mut connection, headers)?;
                    return Ok(());
                }
                let mut bytes = Vec::new();
                let handler = &self.handler;
                let response_writer = ResponseWriter::new(&mut bytes);
                handler(response_writer, request);
                connection.write_all(&bytes)?;
                Ok(())
            }
            Err(err) => {
                let mut bytes = Vec::new();
                let response_writer = ResponseWriter::new(&mut bytes);
                response_writer
                    .write_status_line(StatusCode::BadRequest)
                    .write_default_headers()
                    .write_body_plain_text(&err);
                connection.write_all(&bytes)?;
                 
                Ok(())
            }
        }
    }
    pub fn listen(&self) {
        for stream in self.listener.incoming() {
            println!("new");
            let stream = match stream {
                Ok(my_stream) => my_stream,
                Err(_) => continue,
            };
            if let Err(err) = self.handle(stream) {
                println!("error occurred handling,{err}");
            }
        }
    }
}

pub enum StatusCode {
    Ok,
    BadRequest,
    InternalServerError,
}

fn write_status_line<T: Write>(stream_writer: &mut T, status: StatusCode) -> IoResult<()> {
    let mut status = match status {
        StatusCode::Ok => String::from("HTTP/1.1 200 OK"),
        StatusCode::BadRequest => String::from("HTTP/1.1 400 Bad Request"),
        StatusCode::InternalServerError => String::from("HTTP/1.1 500 Internal Server Error"),
    };
    status.push_str("\r\n");
    stream_writer.write_all(status.as_bytes())?;
    Ok(())
}


fn get_preflight_headers() -> HashMap<&'static str, &'static str> {
    HashMap::from([
        ("Content-Length", "0"),
        ("Content-Type", "text/plain"),
        ("Access-Control-Allow-Origin", "https://hoppscotch.io"),
        ("Access-Control-Allow-Methods", "*"),
        ("Access-Control-Allow-Headers", "*"),
        ("Connection", "close"),
    ])
}

pub fn get_common_headers() -> HashMap<&'static str, &'static str> {
    HashMap::from([
        ("Access-Control-Allow-Origin", "https://hoppscotch.io"),
        ("Connection", "close"),
    ])
}

fn write_headers<T: Write>(stream_writer: &mut T, headers: HashMap<&str, &str>) -> IoResult<()> {
    let mut headers_response = String::new();
    for (key, value) in headers {
        headers_response.push_str(key);
        headers_response.push_str(": ");
        headers_response.push_str(value);
        headers_response.push_str("\r\n");
    }
    headers_response.push_str("\r\n");
    stream_writer.write_all(headers_response.as_bytes())?;
    Ok(())
}
