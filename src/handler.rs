use std::marker::PhantomData;

use crate::{extractor::{FromRequest, FromRequestBody}, parser::http_message_parser::Request, response::{IntoResponse, Response}};

pub trait HandlerFunction<Args>: Send + Sync + 'static+Clone{
    fn execute(&self, request: Request) -> Response;
    
}
impl<F, I,T1, T2, T3> HandlerFunction<(T1,T2, T3,I)> for F
where
    T1: FromRequest,
    T2: FromRequest,
    T3: FromRequestBody,
    I:IntoResponse,
    F: Fn(T1, T2, T3) -> I + Send + Sync + 'static + Clone,
{
    fn execute(
        &self,
        request: Request,
    ) -> Response
    {
        let t1 = match T1::from_request(&request) {
            Ok(val) => val,
            Err(err) => return err.into_response(),
        };
        let t2 = match T2::from_request(&request) {
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

impl<F, I> HandlerFunction<I> for F
where
    I:IntoResponse,
    F: Fn() -> I + Send + Sync + 'static + Clone,
{
    fn execute(
        &self,
        _: Request,
    ) -> Response
    {  
        self().into_response()
    }
}

pub trait Service:Send + Sync + 'static{
    fn call(&self,request: Request)->Response;
    fn clone_box(&self) -> Box<dyn Service>;
}




pub struct Handler<F, Args>
where
    F: HandlerFunction<Args>,
{
    hnd: F,
    _t: PhantomData<Args>,
}

impl<F, Args> Handler<F, Args>
where
    F: HandlerFunction<Args>,
{
    pub fn new(hnd: F) -> Self {
        Handler {
            hnd,
            _t: PhantomData,
        }
    }
}

impl<F, Args> Clone for  Handler<F, Args> 
where
    F: HandlerFunction<Args>,
{
    fn clone(&self) -> Self {
        Handler {
            hnd: self.hnd.clone(),
            _t: PhantomData,
        }
    }
}

impl<F, Args> Service for  Handler<F, Args> 
where
    F: HandlerFunction<Args>,
    Args:Send + Sync + 'static,
{
    fn call(&self,request: Request)->Response {
        self.hnd.execute(request) 
    }
    
    fn clone_box(&self) -> Box<dyn Service> {
        Box::new(self.clone())
    }
}