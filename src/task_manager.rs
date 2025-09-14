use std::{
    io::Result as IoResult,
    net::TcpStream,
    sync::{
        Arc, Mutex,
        mpsc::{self, Receiver, Sender},
    },
    thread::{self, JoinHandle},
};

use crate::{parser::{ first_line_parser::FirstLineRequestParser, http_message_parser::{Parser, Request}}, response_writer::{ContentType, Response, ResponseWriter}, server::{get_preflight_headers, write_headers, write_status_line, StatusCode}};



type Job = Box<dyn FnOnce() + Send + 'static>;
pub struct TaskManager {
    tramsmitter: Option<Sender<Job>>,
    workers: Vec<Worker>,
}

impl TaskManager {
    pub fn new(no_of_threads: usize) -> Self {
        let (tramsmitter, receiver) = mpsc::channel::<Job>();
        let receiver = Arc::new(Mutex::new(receiver));
        let mut workers = Vec::with_capacity(no_of_threads);
        for id in 0..no_of_threads {
            workers.push(Worker::new(id, receiver.clone()));
        }

        Self {
            workers,
            tramsmitter: Some(tramsmitter),
        }
    }
    pub fn execute<F>(&self, function_to_execute: F)
    where
        F: FnOnce() + Send + 'static,
    {
        let job = Box::new(function_to_execute);
        self.tramsmitter.as_ref().unwrap().send(job).unwrap();
    }
}
impl Drop for TaskManager {
    fn drop(&mut self) {
        drop(self.tramsmitter.take());
        for worker in self.workers.drain(..) {
            worker.task_handle.join().unwrap();
            println!("shut down worker thread :{}", worker.id);
        }
    }
}

struct Worker {
    id: usize,
    task_handle: JoinHandle<()>,
}
impl Worker {
    fn new(id: usize, receiver: Arc<Mutex<Receiver<Job>>>) -> Self {
        let task_handle = thread::spawn(move || {
            loop {
                let job = {
                    match receiver.lock().unwrap().recv() {
                        Ok(job) => job,
                        Err(_) => {
                            println!("ending thread{id}");
                            break;
                        },
                    }
                }; // Lock is released here!
                
                // Now execute the job without holding the lock
                job();
                println!("served by thread {}", id);
            }
        });
        Self { id, task_handle }
    }
}

pub fn handle<F>(mut connection: TcpStream, custom_handler: Arc<F>) -> IoResult<()>
where
    F: Fn(ResponseWriter, Request) -> IoResult<Response>,
{
    let request_parser = Parser::new(FirstLineRequestParser::default());
    match request_parser.parse(&mut connection) {
        Ok(payload_request) => {
            let request=Request::from(payload_request);
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


