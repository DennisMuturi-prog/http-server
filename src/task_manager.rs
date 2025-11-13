use std::{
    io::{Result as IoResult, Write},
    net::TcpStream,
    sync::{
        mpsc::{self, Receiver, Sender}, Arc, Mutex
    },
    thread::{self, JoinHandle},
};


use crate::{parser::{ first_line_parser::FirstLineRequestParser, http_message_parser::{Parser, Request}}, response::{get_common_headers, get_preflight_headers, write_headers, write_response_headers, write_response_status_line, write_status_line, ContentType, Response, StatusCode}, response_writer::ResponseWriter, routing::{HttpVerb, RoutingMap}};



type Job = Box<dyn FnOnce() + Send + 'static>;
pub struct TaskManager {
    tramsmitter: Option<Sender<Job>>,
    workers: Vec<Worker>,
}

impl TaskManager {
    pub fn new(no_of_threads: usize) -> Self
    {
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

pub fn handle(mut connection: TcpStream, custom_handler: Arc<RoutingMap>) -> IoResult<()>
{
    let request_parser = Parser::new(FirstLineRequestParser::default());
    match request_parser.parse(&mut connection) {
        Ok(payload_request) => {
            let request=Request::from(payload_request);
            if request.request_method() == HttpVerb::OPTIONS {
                write_status_line(&mut connection, StatusCode::Ok)?;
                let headers = get_preflight_headers();
                write_headers(&mut connection, headers)?;
                return Ok(());

            }
            let handler_function=match custom_handler.get_handler(&request.request_method(), request.request_path()){
                Some(val) => val,
                None => {
                    write_status_line(&mut connection, StatusCode::NotFound)?;
                    let headers = get_common_headers();
                    write_headers(&mut connection, headers)?;
                    return Ok(());

                },
            };
            let sending_response=handler_function.call(request, custom_handler);
            send_response_to_network(connection, sending_response)?;
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

fn send_response_to_network(mut connection:TcpStream,sending_response:Response)->IoResult<()>{
    write_response_status_line(&mut connection,sending_response.status_code() )?;
    write_response_headers(&mut connection, sending_response.headers())?;
    if !sending_response.body().is_empty(){
        connection.write_all(sending_response.body())?;
    }
    Ok(())
}


