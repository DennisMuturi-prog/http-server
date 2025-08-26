use std::{collections::HashMap, fs::File, io::{Result as IoResult, Write}};

use single_threaded_server::{request_parser::Request, response_writer::{Response, ResponseWriter}, server::{Server, StatusCode}
};

fn main() -> IoResult<()> {
    let server = Server::serve(8000, handler)?;
    server.proxy_listen();
    Ok(())
}

fn handler(response_writer: ResponseWriter, request: Request) -> IoResult<Response> {
    println!("request:{:?}",request.headers());
    let request_path = request.request_path();
    let content_type_header=match request.header("content-type"){
        Some(accept) => accept,
        None => "",
    };
    if content_type_header=="image/jpeg"{
        println!("hello");
        let mut file=File::create("muturi.jpg").unwrap();
        file.write_all(request.body()).unwrap();
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


