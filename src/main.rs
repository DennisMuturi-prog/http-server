use std::{io::Result as IoResult};

use single_threaded_server::{request_parser::Request, response_writer::{Response, ResponseWriter}, server::{ get_common_headers, Server, StatusCode}};

fn main() -> IoResult<()> {
    let server=Server::serve(8000,handler)?;
    server.listen();
    Ok(())
}

fn handler(response_writer:ResponseWriter,request:Request)->Response{
    let request_path=request.get_request_path();
    if request_path=="/yourproblem"{
        let response_message="Your problem is not my problem\n";
        let default_headers=get_common_headers();
        response_writer.write_status_line(StatusCode::BadRequest).write_headers(default_headers).write_body_plain_text(response_message)
        

    }else if request_path=="/myproblem"{
        let response_message="Woopsie, my bad\n";
        let default_headers=get_common_headers();
        response_writer.write_status_line(StatusCode::InternalServerError).write_headers(default_headers).write_body_plain_text(response_message)

    }else{
        let response_message="<h1>Hello world</h1>";
        let default_headers=get_common_headers();
        response_writer.write_status_line(StatusCode::Ok).write_headers(default_headers).write_body_html(response_message)
        
    }

}







