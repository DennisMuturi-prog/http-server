use std::collections::HashMap;

use matchit::Router;

use crate::handler::{Handler, HandlerFunction, Service};


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
    pub fn add_handler<Args,F>(
        &mut self,
        http_verb: HttpVerb,
        handler: F,
        route: &'static str,
    ) -> Result<(), matchit::InsertError>
    where
    F: HandlerFunction<Args>,
    Args:Send + Sync +'static
    {
        let router = self.0.entry(http_verb).or_default();
        router.insert(route, Box::new(Handler::new(handler)))?;
        Ok(())
    }
    pub fn get_handler(&self,http_verb:&HttpVerb,route:&str) ->Option<&dyn Service> 
    
    {
        let method_route=self.0.get(http_verb)?;
        let matched_route = method_route.at(route).ok()?;
        let handler_function=matched_route.value;
        Some(handler_function.as_ref())
    }
    pub fn get_method_router(&self,http_verb: &HttpVerb)->Option<&Router<Box<dyn Service>>>
    {
        self.0.get(http_verb)

    }
}














