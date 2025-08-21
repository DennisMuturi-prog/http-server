use std::{
    collections::HashMap,
    io::{Result as IoResult, Write},
    net::{TcpListener, TcpStream},
};

use crate::{
    request_parser::{Request, request_from_reader},
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
    pub fn serve(port: usize, handler: F) -> IoResult<Self> {
        let formatted_addr = format!("127.0.0.1:{port}");
        let listener = TcpListener::bind(&formatted_addr)?;
        Ok(Server { listener, handler })
    }
    fn handle(&self, mut connection: TcpStream) -> IoResult<()> {
        match request_from_reader(&mut connection) {
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
                return Ok(());
            }
            Err(_) => {
                write_status_line(&mut connection, StatusCode::BadRequest)?;
                let headers = get_default_headers();
                write_headers(&mut connection, headers)?;
                return Ok(());
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

fn get_default_headers() -> HashMap<&'static str, &'static str> {
    HashMap::from([
        ("Content-Length", "0"),
        ("Content-Type", "text/plain"),
        ("Access-Control-Allow-Origin", "https://hoppscotch.io"),
        ("Connection", "close"),
    ])
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

pub fn get_common_headers() -> HashMap<String, String> {
    HashMap::from([
        (
            "Access-Control-Allow-Origin".to_string(),
            "https://hoppscotch.io".to_string(),
        ),
        ("Connection".to_string(), "close".to_string()),
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
