use std::{collections::HashMap, io::{Result as IoResult, Write}, net::{ TcpStream, ToSocketAddrs}};

use single_threaded_server::{
    chunked_parsing::response_from_reader, general_parser::response_from_reader_general, request_parser::Request, response_writer::{Response, ResponseWriter}, server::{Server, StatusCode}
};

fn main() -> IoResult<()> {
    // let server = Server::serve(8000, handler)?;
    // server.listen();
    connect_to_remote().unwrap();
    Ok(())
}

fn handler(response_writer: ResponseWriter, request: Request) -> Response {
    let request_path = request.get_request_path();
    if request_path == "/yourproblem" {
        let response_message = "Your problem is not my problem\n";
        response_writer
            .write_status_line(StatusCode::BadRequest)
            .write_default_headers()
            .write_body_plain_text(response_message)
    } else if request_path == "/myproblem" {
        let response_message = "Woopsie, my bad\n";
        response_writer
            .write_status_line(StatusCode::InternalServerError)
            .write_default_headers()
            .write_body_plain_text(response_message)
    } else {
        let response_message = "<h1>Hello world</h1>";
        let custom_headers = HashMap::from([
            ("Access-Control-Allow-Origin", "https://hoppscotch.io"),
            ("Connection", "alive"),
        ]);
        response_writer
            .write_status_line(StatusCode::Ok)
            .write_headers(custom_headers)
            .write_body_html(response_message)
    }
}


fn connect_to_remote()->IoResult<()>{
    let host = "httpbin.org:80";
    let ip_lookup = host.to_socket_addrs()?.next().unwrap();
    let mut connection=TcpStream::connect(ip_lookup).unwrap();
    connection.write_all(b"GET /stream/10 HTTP/1.1\r\nAccept: */*\r\nHost: httpbin.org\r\nContent-Length:0\r\n\r\n")?;
    let response=response_from_reader_general(&mut connection);
    println!("response is {:?}",response);
    Ok(())
}