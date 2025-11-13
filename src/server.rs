use std::{
    io::Result as IoResult, net::{SocketAddr, TcpListener, TcpStream, ToSocketAddrs}, sync::Arc
};
use crate::{
    extractor::IntoResponse, parser:: first_line_parser::{FirstLineRequestParser, FirstLineResponseParser}, proxy::{ProxyParser, RequestPartProxySender, ResponsePartProxySender}, routing::{HandlerFunction, HttpVerb, RoutingMap}, task_manager::{TaskManager, handle}
};

pub struct Server {
    listener: TcpListener,
    no_of_threads: usize,
    router:RoutingMap
}

impl Server
{
    pub fn serve(port: u16, no_of_threads: usize) -> IoResult<Self> {
        let listener = TcpListener::bind(SocketAddr::from(([127, 0, 0, 1], port)))?;
        Ok(Server {
            listener,
            no_of_threads,
            router:RoutingMap::new()
        })
    }
    pub fn post<Args,I,F>(&mut self,route:&'static str,handler: F)->Result<(),matchit::InsertError>
    where F:HandlerFunction<Args,I>,
    I:Send + Sync +'static,
    I:IntoResponse,
    Args:Send + Sync +'static{
        self.router.add_handler(HttpVerb::POST, handler, route)?;
        Ok(())
    }
    pub fn get<Args,I,F>(&mut self,route:&'static str,handler: F)->Result<(),matchit::InsertError>
    where F:HandlerFunction<Args,I>,
    I:Send + Sync +'static,
    Args:Send + Sync +'static,
    I:IntoResponse{
        self.router.add_handler(HttpVerb::GET, handler, route)?;
        Ok(())
    }
    pub fn delete<Args,I,F>(&mut self,route:&'static str,handler: F)->Result<(),matchit::InsertError>
    where F:HandlerFunction<Args,I>,
    I:IntoResponse+Send + Sync +'static,
    Args:Send + Sync +'static{
        self.router.add_handler(HttpVerb::DELETE, handler, route)?;
        Ok(())
    }
    pub fn listen(self) 
    {
        let task_manager = TaskManager::new(self.no_of_threads);
        let routing_map=Arc::new(self.router);
        for stream in self.listener.incoming() {
            println!("new");
            let stream = match stream {
                Ok(my_stream) => my_stream,
                Err(_) => continue,
            };
            let global_router=Arc::clone(&routing_map);
            task_manager.execute(|| {
                if let Err(err) = handle(stream,global_router ) {
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
