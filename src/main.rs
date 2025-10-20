use std::{collections::HashMap, fs::File, io::{Read, Result as IoResult, Write}, thread::sleep, time};

use serde::Deserialize;
use single_threaded_server::{extractor::{Json, Path, Query}, parser::http_message_parser::Request, response::{SendingResponse, StatusMessage}, response_writer::{ContentType, Response, ResponseWriter}, server::{ get_common_headers_with_content, Server, StatusCode}
};

fn main() -> IoResult<()> {
    
    let mut server = Server::serve(8000, 10)?;
    server.post("/{id}/{name}/hello", new_handler).unwrap();
    server.listen();
    Ok(())
}

fn new_handler(Query(user):Query<User>,Json(user2):Json<User>,Path(user_info):Path<UserInfo>)->SendingResponse{
    println!("json username is {} and password is {}",user2.username,user2.password);
    println!("query username is {} and password is {}",user.username,user.password);
    println!("path id is {} and name is {}",user_info.id,user_info.name);
    let message=b"hello";
    SendingResponse::new(StatusMessage::Accepted,StatusCode::Ok,get_common_headers_with_content(message),message.to_vec())

}
#[derive(Deserialize)]
struct User{
    username:String,
    password:String,
}

#[derive(Deserialize)]
struct UserInfo{
    id:u32,
    name:String,
}

fn handler(response_writer: ResponseWriter, request: Request) -> IoResult<Response> {
    println!("request:{:?}",request.request_path());
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
    if request_path=="/favicon.ico"{
        send_image_in_chunks(response_writer)
    }
    else if request_path == "/yourproblem" {
        let response_message = "Your problem is not my problem\n";
        response_writer
            .write_status_line(StatusCode::BadRequest)?
            .write_default_headers(ContentType::TextPlain)?
            .write_body_plain_text(response_message)
    } 
    else if request_path == "/myproblem" {
        let response_message = "Woopsie, my bad\n";
        response_writer
            .write_status_line(StatusCode::InternalServerError)?
            .write_default_headers(ContentType::TextPlain)?
            .write_body_plain_text(response_message)
    } 
    else if request_path == "/sleep" {
        sleep(time::Duration::from_secs(20));
        let response_message = "What a nap 😴🥱\n";
        response_writer
            .write_status_line(StatusCode::InternalServerError)?
            .write_default_headers(ContentType::TextPlain)?
            .write_body_plain_text(response_message)
    }else {
        let response_message = "<h1>Hello world 👀</h1><a href=\"/favicon.ico\" download>Download image</a>";
        let custom_headers = HashMap::from([
            ("Access-Control-Allow-Origin", "https://hoppscotch.io"),
            ("Connection", "keep-alive"),
        ]);
        response_writer
            .write_status_line(StatusCode::Ok)?
            .write_headers(custom_headers,ContentType::TextHtml)?
            .write_body_html(response_message)
    }
}


fn send_image_in_chunks(response_writer: ResponseWriter)->IoResult<Response>{
    let mut buf=[0;1024];
    let mut file=File::open("muturi.jpg")?;
    let mut response_writer=response_writer.write_status_line(StatusCode::Ok)?.write_default_headers(ContentType::ImageJpeg)?;


    loop{
        let n=file.read(&mut buf)?;
        if n==0{
            return response_writer.write_chunked_body_done();
        }
        response_writer.write_chunk(&buf[..n])?;
    }

}


