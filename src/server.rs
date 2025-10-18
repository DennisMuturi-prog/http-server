use std::{
    collections::HashMap,
    io::{Result as IoResult, Write},
    net::{SocketAddr, TcpListener, TcpStream, ToSocketAddrs},
    sync::Arc,
};

use crate::{
    parser::{ first_line_parser::{FirstLineRequestParser, FirstLineResponseParser, ResponseLine}, http_message_parser::Request}, proxy::{ProxyParser, RequestPartProxySender, ResponsePartProxySender}, response_writer::{ Response, ResponseWriter}, task_manager::{handle, TaskManager}
};

pub struct Server<F> {
    listener: TcpListener,
    handler: Arc<F>,
    no_of_threads: usize,
}

impl<F> Server<F>
where
    F: Fn(ResponseWriter, Request) -> IoResult<Response>+ Send + Sync +'static,
{
    pub fn serve(port: u16, no_of_threads: usize, handler: F) -> IoResult<Self> {
        let listener = TcpListener::bind(SocketAddr::from(([127, 0, 0, 1], port)))?;
        Ok(Server {
            listener,
            handler:Arc::new(handler),
            no_of_threads,
        })
    }
    pub fn listen(self) {
        let task_manager = TaskManager::new(self.no_of_threads);
        for stream in self.listener.incoming() {
            println!("new");
            let stream = match stream {
                Ok(my_stream) => my_stream,
                Err(_) => continue,
            };
            let custom_handler = Arc::clone(&self.handler);
            task_manager.execute(|| {
                if let Err(err) = handle(stream, custom_handler) {
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




fn proxy_to_remote(mut client_stream: TcpStream) -> IoResult<()> {
    let host = "httpbin.org:80";
    let ip_lookup = host.to_socket_addrs()?.next().unwrap();
    let mut connection = TcpStream::connect(ip_lookup).unwrap();
    let mut request_parser = ProxyParser::new(FirstLineRequestParser::default(),&mut connection,RequestPartProxySender::new(host));
    request_parser
        .parse(&mut client_stream)
        .unwrap();
    let mut response_parser = ProxyParser::new(FirstLineResponseParser::default(),&mut client_stream,ResponsePartProxySender{});
    response_parser
        .parse(&mut connection)
        .unwrap();
    Ok(())
}
