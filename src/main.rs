use std::io::{ Result as IoResult};

use serde::{Deserialize, Serialize};
use single_threaded_server::{extractor::{Form, IntoResponse, Json, Path, Query}, server::Server
};

fn main() -> IoResult<()> {
    
    let mut server = Server::serve(8000, 10)?;
    server.post("/{id}/{name}/hello", new_handler).unwrap();
    server.listen();
    Ok(())
}

fn new_handler(Query(user):Query<User>,Path(user_info):Path<UserInfo>,Form(user2):Form<User>)->impl IntoResponse{
    println!("json username is {} and password is {}",user2.username,user2.password);
    println!("query username is {} and password is {}",user.username,user.password);
    println!("path id is {} and name is {}",user_info.id,user_info.name);
    let new_username=User{
        username:"hello".to_string(),
        password:"world".to_string()
    };
    Json(new_username)
    // let message=b"hello";
    // SendingResponse::new(StatusMessage::Accepted,StatusCode::Ok,get_common_headers_with_content_type_header(message,ContentType::TextPlain),message.to_vec())

}
#[derive(Serialize,Deserialize)]
struct User{
    username:String,
    password:String,
}

#[derive(Deserialize)]
struct UserInfo{
    id:u32,
    name:String,
}







