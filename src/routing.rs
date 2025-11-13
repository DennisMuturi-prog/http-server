use std::{collections::HashMap, marker::PhantomData, sync::Arc};

use matchit::Router;

use crate::{
    extractor::{FromRequest, FromRequestBody, FromRoutingMap, IntoResponse},
    parser::http_message_parser::Request,
    response::Response,
};

#[derive(Eq, Hash, PartialEq)]
pub enum HttpVerb {
    GET,
    POST,
    PUT,
    PATCH,
    DELETE,
    OPTIONS,
    HEAD,
}



#[derive(Default)]
pub struct RoutingMap(HashMap<HttpVerb, Router<Box<dyn Service>>>);

impl RoutingMap {
    pub fn new() -> Self {
        Self(HashMap::new())
    }
    pub fn add_handler<Args,I,F>(
        &mut self,
        http_verb: HttpVerb,
        handler: F,
        route: &'static str,
    ) -> Result<(), matchit::InsertError>
    where
    F: HandlerFunction<Args,I>,
    I:IntoResponse+Send + Sync +'static,
    Args:Send + Sync +'static
    {
        let router = self.0.entry(http_verb).or_default();
        router.insert(route, Box::new(Handler::new(handler)))?;
        Ok(())
    }
    pub fn get_handler(&self,http_verb:&HttpVerb,route:&str) ->Option<Box<dyn Service>> 
    
    {
        let method_route=self.0.get(http_verb)?;
        let matched_route = method_route.at(route).ok()?;
        let handler_function=matched_route.value;
        Some(handler_function.clone_box())
    }
    pub fn get_method_router(&self,http_verb: &HttpVerb)->Option<&Router<Box<dyn Service>>>
    {
        self.0.get(http_verb)

    }
}
pub trait HandlerFunction<Args,I>: Send + Sync + 'static+Clone{
    fn execute(&self, request: Request, routing_map: Arc<RoutingMap>) -> Response;
    
}
impl<F, I,T1, T2, T3> HandlerFunction<(T1,T2, T3),I> for F
where
    T1: FromRequest,
    T2: FromRoutingMap,
    T3: FromRequestBody,
    I:IntoResponse,
    F: Fn(T1, T2, T3) -> I + Send + Sync + 'static + Clone,
{
    fn execute(
        &self,
        request: Request,
        routing_map: Arc<RoutingMap>,
    ) -> Response
    {
        let t1 = match T1::from_request(&request) {
            Ok(val) => val,
            Err(err) => return err.into_response(),
        };
        let t2 = match T2::from_routing_map(&request,Arc::clone(&routing_map)) {
            Ok(val) => val,
            Err(err) => return err.into_response(),
        };
        let t3 = match T3::from_request_body(&request) {
            Ok(val) => val,
            Err(err) => return err.into_response(),
        };
        self(t1, t2, t3).into_response()
    }
}

impl<F, I> HandlerFunction<(),I> for F
where
    I:IntoResponse,
    F: Fn() -> I + Send + Sync + 'static + Clone,
{
    fn execute(
        &self,
        _: Request,
        _: Arc<RoutingMap>,
    ) -> Response
    {  
        self().into_response()
    }
}

pub trait Service:Send + Sync + 'static{
    fn call(&self,request: Request,routing_map: Arc<RoutingMap>)->Response;
    fn clone_box(&self) -> Box<dyn Service>;
}




pub struct Handler<F,I, T>
where
    F: HandlerFunction<T, I>,
    I: IntoResponse,
{
    hnd: F,
    _t: PhantomData<(T,I)>,
}

impl<F,I, T> Handler<F,I, T>
where
    F: HandlerFunction<T, I>,
    I: IntoResponse,
{
    pub fn new(hnd: F) -> Self {
        Handler {
            hnd,
            _t: PhantomData,
        }
    }
}

impl<F,I, T> Clone for  Handler<F,I, T> 
where
    F: HandlerFunction<T, I>,
    I: IntoResponse
{
    fn clone(&self) -> Self {
        Handler {
            hnd: self.hnd.clone(),
            _t: PhantomData,
        }
    }
}

impl<F,I, T> Service for  Handler<F,I, T> 
where
    F: HandlerFunction<T, I>,
    T:Send + Sync + 'static,
    I: IntoResponse+'static+Send + Sync
{
    fn call(&self,request: Request,routing_map: Arc<RoutingMap>)->Response {
        self.hnd.execute(request, routing_map) 
    }
    
    fn clone_box(&self) -> Box<dyn Service> {
        Box::new(self.clone())
    }
}













