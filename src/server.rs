use std::{
    collections::HashMap,
    io::{Result as IoResult, Write},
    net::{SocketAddr, TcpListener, TcpStream, ToSocketAddrs},
    sync::Arc,
};

use crate::{
    http_message_parser::HttpMessage,
    proxy_request_parser::ProxyRequestParser,
    proxy_response_parser::ProxyResponseParser,
    request_parser::{Request, RequestLine, RequestParser},
    response_parser::ResponseLine,
    response_writer::{ContentType, Response, ResponseWriter},
    task_manager::{ handle, TaskManager},
};

pub struct Server<F> {
    listener: TcpListener,
    handler: F,
    no_of_threads:usize
}

impl<F> Server<F>
where
    F: Fn(ResponseWriter, Request)-> IoResult<Response> + Send + 'static + Sync
{
    pub fn serve(port: u16,no_of_threads:usize, handler: F) -> IoResult<Self> {
        let listener = TcpListener::bind(SocketAddr::from(([127, 0, 0, 1], port)))?;
        Ok(Server { listener, handler,no_of_threads })
    }
    fn handle(&self, mut connection: TcpStream) -> IoResult<()> {
        let mut request_parser = RequestParser::default();
        match request_parser.http_message_from_reader(&mut connection) {
            Ok(request) => {
                if request.request_method() == "OPTIONS" {
                    write_status_line(&mut connection, StatusCode::Ok)?;
                    let headers = get_preflight_headers();
                    write_headers(&mut connection, headers)?;
                    return Ok(());
                }
                let handler = &self.handler;
                let response_writer = ResponseWriter::new(&mut connection);
                handler(response_writer, request)?;
                Ok(())
            }
            Err(err) => {
                if err == "false alarm" {
                    connection.shutdown(std::net::Shutdown::Both)?;
                    return Ok(());
                }
                let response_writer = ResponseWriter::new(&mut connection);
                response_writer
                    .write_status_line(StatusCode::BadRequest)?
                    .write_default_headers(ContentType::TextPlain)?
                    .write_body_plain_text(&err)?;
                Ok(())
            }
        }
    }
    pub fn blocking_listen(&self) {
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

    pub fn listen(self) {
        let task_manager = TaskManager::new(self.no_of_threads);
        let handler = Arc::new(self.handler); 
        for stream in self.listener.incoming() {
            println!("new");
            let stream = match stream {
                Ok(my_stream) => my_stream,
                Err(_) => continue,
            };
            let custom_handler=handler.clone();
            task_manager.execute(move || {
                if let Err(err) = handle(stream,custom_handler) {
                    println!("error occurred handling,{err}");
                }
            });
        }
    }
    pub fn proxy_listen(&self) {
        for stream in self.listener.incoming() {
            println!("new");
            let stream = match stream {
                Ok(my_stream) => my_stream,
                Err(_) => continue,
            };
            if let Err(err) = proxy_to_remote(stream) {
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

pub fn write_status_line<T: Write>(stream_writer: &mut T, status: StatusCode) -> IoResult<()> {
    let mut status = match status {
        StatusCode::Ok => String::from("HTTP/1.1 200 OK"),
        StatusCode::BadRequest => String::from("HTTP/1.1 400 Bad Request"),
        StatusCode::InternalServerError => String::from("HTTP/1.1 500 Internal Server Error"),
    };
    status.push_str("\r\n");
    stream_writer.write_all(status.as_bytes())?;
    Ok(())
}

pub fn write_proxied_request_line<T: Write>(
    stream_writer: &mut T,
    request: &RequestLine,
    remote_host: &str,
) -> IoResult<()> {
    let status_line = format!(
        "{} {} HTTP/1.1\r\nHost: {}\r\n",
        request.method(),
        request.request_target(),
        remote_host
    );
    stream_writer.write_all(status_line.as_bytes())?;
    Ok(())
}

pub fn write_proxied_response_status_line<T: Write>(
    stream_writer: &mut T,
    response: &ResponseLine,
) -> IoResult<()> {
    let status_line = format!(
        "HTTP/1.1 {} {}\r\n",
        response.status_code(),
        response.status_message()
    );
    stream_writer.write_all(status_line.as_bytes())?;
    Ok(())
}

pub fn get_preflight_headers() -> HashMap<&'static str, &'static str> {
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
        ("Connection", "keep-alive"),
    ])
}

pub fn write_headers<T: Write>(
    stream_writer: &mut T,
    headers: HashMap<&str, &str>,
) -> IoResult<()> {
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
pub fn write_proxied_headers<T: Write>(
    stream_writer: &mut T,
    headers: &HashMap<String, String>,
) -> IoResult<()> {
    let mut headers_response = String::new();
    for (key, value) in headers {
        if key == "host" {
            continue;
        }
        headers_response.push_str(key.as_str());
        headers_response.push_str(": ");
        headers_response.push_str(value.as_str());
        headers_response.push_str("\r\n");
    }
    headers_response.push_str("\r\n");
    stream_writer.write_all(headers_response.as_bytes())?;
    Ok(())
}

fn proxy_to_remote(mut client_stream: TcpStream) -> IoResult<()> {
    let host = "httpbin.org:80";
    let ip_lookup = host.to_socket_addrs()?.next().unwrap();
    let mut connection = TcpStream::connect(ip_lookup).unwrap();
    let mut request_parser = ProxyRequestParser::new(&mut connection, "httpbin.org");
    request_parser
        .http_message_from_reader(&mut client_stream)
        .unwrap();
    let mut response_parser = ProxyResponseParser::new(&mut client_stream);
    let response = response_parser
        .http_message_from_reader(&mut connection)
        .unwrap();
    let parsed_body = String::from_utf8(response.body().to_vec()).unwrap();
    println!("response is \n{}", parsed_body);
    Ok(())
}
