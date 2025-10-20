use std::{collections::HashMap};

use matchit::Router;
use thiserror::Error;

use crate::{extractor::{FromRequest, FromRoutingMap, IntoResponse}, parser::http_message_parser::Request, response_writer::Response};


#[derive(Eq, Hash, PartialEq)]
pub enum HttpVerb{
    GET,
    POST,
    PUT,
    PATCH,
    DELETE,
    OPTIONS,
    HEAD
}
pub trait HandlerFunction<Args>:Send + Sync + 'static{
    fn execute(self,request:Request)->Result<Response,ExtractionError>;
}
pub struct RoutingMap<F>(HashMap<HttpVerb,Router<F>>);

impl<F> RoutingMap<F> {
    pub fn new()->Self{
        Self(HashMap::new())
    }
    pub fn add_handler<Args>(&mut self,http_verb:HttpVerb,handler:F,route:&'static str)->Result<(),matchit::InsertError>
    where F:HandlerFunction<Args>
    {
        let router=self.0.entry(http_verb).or_default();
        router.insert(route, handler)?;
        Ok(())
    }
}
impl<F,T1,T2,T3,T4> HandlerFunction<(T1,T2,T3,T4)> for F where
T1:FromRequest,
T2:FromRequest,
T3:FromRequest,
T4:FromRoutingMap,
F:Fn(T1,T2,T3,T4)->Response+Send+Sync+'static,
{
    fn execute(self,request:Request)->Result<Response,ExtractionError> {
        let t1=T1::from_request(&request)?;
        let t2=T2::from_request(&request)?;
        let t3=T3::from_request(&request)?;
    }
}

#[derive(Error, Debug)]
enum ExtractionError {
    #[error("route handler not found")]
    RoutingError,
    #[error("failed to deserialize body json {0}")]
    SerdeJsonError(#[from] serde_urlencoded::de::Error)
}
impl IntoResponse for ExtractionError{}