use std::{collections::HashMap, fs::File, io::{Result as IoResult, Write}, net::{ TcpStream, ToSocketAddrs}};

use single_threaded_server::{http_message_parser::HttpMessage, proxy_response_parser::ProxyResponseParser, request_parser::Request, response_parser::ResponseParser, response_writer::{Response, ResponseWriter}, server::{Server, StatusCode}
};

fn main() -> IoResult<()> {
    let server = Server::serve(8000, handler)?;
    server.listen();
    // connect_to_remote().unwrap();
    Ok(())
}

fn handler(response_writer: ResponseWriter, request: Request) -> IoResult<Response> {
    println!("request:{:?}",request.get_all_headers());
    let request_path = request.get_request_path();
    let content_type_header=match request.get_header("content-type"){
        Some(accept) => accept,
        None => "",
    };
    if content_type_header=="image/jpeg"{
        println!("hello");
        let mut file=File::create("muturi.jpg").unwrap();
        file.write_all(request.get_body()).unwrap();
    }
    if request_path == "/yourproblem" {
        let response_message = "Your problem is not my problem\n";
        response_writer
            .write_status_line(StatusCode::BadRequest)?
            .write_default_headers()?
            .write_body_plain_text(response_message)
    } else if request_path == "/myproblem" {
        let response_message = "Woopsie, my bad\n";
        response_writer
            .write_status_line(StatusCode::InternalServerError)?
            .write_default_headers()?
            .write_body_plain_text(response_message)
    } else {
        let response_message = "<h1>Hello world</h1>";
        let custom_headers = HashMap::from([
            ("Access-Control-Allow-Origin", "https://hoppscotch.io"),
            ("Connection", "alive"),
        ]);
        response_writer
            .write_status_line(StatusCode::Ok)?
            .write_headers(custom_headers)?
            .write_body_html(response_message)
    }
}


