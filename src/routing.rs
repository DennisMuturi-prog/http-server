use std::{collections::HashMap, sync::Arc};

use matchit::Router;

use crate::{
    extractor::{FromRequest, FromRequestBody, FromRoutingMap, IntoResponse},
    parser::http_message_parser::Request,
    response::SendingResponse,
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
pub struct RoutingMap<F>(HashMap<HttpVerb, Router<F>>);

impl<F> RoutingMap<F> {
    pub fn new() -> Self {
        Self(HashMap::new())
    }
    pub fn add_handler<Args>(
        &mut self,
        http_verb: HttpVerb,
        handler: F,
        route: &'static str,
    ) -> Result<(), matchit::InsertError>
    where
    F: HandlerFunction<Args>,
    {
        let router = self.0.entry(http_verb).or_default();
        router.insert(route, handler)?;
        Ok(())
    }
    pub fn get_handler<Args>(&self,http_verb:&HttpVerb,route:&str) ->Option<F> 
    where
    F: HandlerFunction<Args>,
    {
        let method_route=self.0.get(http_verb)?;
        let matched_route = method_route.at(route).ok()?;
        let handler_function=matched_route.value;
        Some(handler_function.clone())
    }
    pub fn get_method_router<Args>(&self,http_verb: &HttpVerb)->Option<&Router<F>>
    where F: HandlerFunction<Args>{
        self.0.get(http_verb)

    }
}
pub trait HandlerFunction<Args>: Send + Sync + 'static+Clone{
    fn execute<F2, Args2>(self, request: Request, routing_map: Arc<RoutingMap<F2>>) -> SendingResponse
    where
        F2: HandlerFunction<Args2>;
}
impl<F, I,T1, T2, T3> HandlerFunction<(T1, I,T2, T3)> for F
where
    T1: FromRequest,
    T2: FromRoutingMap,
    T3: FromRequestBody,
    I:IntoResponse,
    F: Fn(T1, T2, T3) -> I + Send + Sync + 'static + Clone,
{
    fn execute<F2, Args>(
        self,
        request: Request,
        routing_map: Arc<RoutingMap<F2>>,
    ) -> SendingResponse
    where
        F2: HandlerFunction<Args>,
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

// #[derive(Error, Debug)]
// enum ExtractionError {
//     #[error("route handler not found")]
//     RoutingError,
//     #[error("failed to deserialize body json {0}")]
//     SerdeJsonError(#[from] serde_urlencoded::de::Error)
// }
// impl IntoResponse for ExtractionError{}
