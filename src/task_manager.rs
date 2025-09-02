use std::{
    io::{Result as IoResult},
    net::TcpStream,
    sync::{
        Arc, Mutex,
        mpsc::{self, Receiver, Sender},
    },
    thread::{self, JoinHandle},
};

use crate::{
    http_message_parser::HttpMessage,
    request_parser::{Request, RequestParser},
    response_writer::{ContentType, Response, ResponseWriter},
    server::{StatusCode, get_preflight_headers, write_headers, write_status_line},
};

type Job = Box<dyn FnOnce() + Send + 'static>;
pub struct TaskManager {
    tramsmitter: Sender<Job>,
    tasks: Vec<Task>,
}

impl TaskManager {
    pub fn new(no_of_threads: usize) -> Self {
        let (tramsmitter, receiver) = mpsc::channel::<Job>();
        let receiver = Arc::new(Mutex::new(receiver));
        let mut tasks = Vec::with_capacity(no_of_threads);
        for id in 0..no_of_threads {
            tasks.push(Task::new(id,receiver.clone()));
        }

        Self { tasks, tramsmitter }
    }
    pub fn execute<F>(&self, function_to_execute: F)
    where
        F: FnOnce() + Send + 'static,
    {
        let job=Box::new(function_to_execute);
        self.tramsmitter.send(job).unwrap();
    }
}

struct Task {
    id:usize,
    task_handle: JoinHandle<()>,
}
impl Task {
    pub fn new(id:usize,receiver: Arc<Mutex<Receiver<Job>>>) -> Self{
    
        let task_handle = thread::spawn(move || {
            loop {
                let job = receiver.lock().unwrap().recv().unwrap();
                job();
                println!("served by thread {}",id);
            }
        });
        Self {id, task_handle }
    }
}

pub fn handle<F>(mut connection: TcpStream,custom_handler:Arc<F>) -> IoResult<()> 
where F: Fn(ResponseWriter, Request) -> IoResult<Response>{
    let mut request_parser = RequestParser::default();
    match request_parser.http_message_from_reader(&mut connection) {
        Ok(request) => {
            if request.request_method() == "OPTIONS" {
                write_status_line(&mut connection, StatusCode::Ok)?;
                let headers = get_preflight_headers();
                write_headers(&mut connection, headers)?;
                return Ok(());
            }
            let response_writer = ResponseWriter::new(&mut connection);
            custom_handler(response_writer, request)?;
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

