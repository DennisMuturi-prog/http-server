use std::{ sync::Arc};

use serde::{Deserialize, de::DeserializeOwned};
use thiserror::Error;


use crate::{
    parser::http_message_parser::Request, response::{SendingResponse, StatusMessage}, routing::{HandlerFunction, RoutingMap}, server::{get_common_headers_with_content, StatusCode}
};

pub struct Json<T>(pub T);

pub trait FromRequest {
    type Error:IntoResponse;
    fn from_request(request: &Request) -> Result<Self, Self::Error>
    where
        Self: std::marker::Sized;
}

impl<T> FromRequest for Json<T>
where
    T: DeserializeOwned,
{
    type Error = serde_json::Error;
    fn from_request(request: &Request) -> Result<Self, Self::Error> {
        let result: T = serde_json::from_slice(request.body())?;
        Ok(Json(result))
    }
}

pub struct Query<T>(pub T);

impl<T> FromRequest for Query<T>
where
    T: DeserializeOwned,
{
    type Error = serde_urlencoded::de::Error;
    fn from_request(request: &Request) -> Result<Self, Self::Error> {
        let result: T = serde_urlencoded::from_str(request.query_params_string())?;
        Ok(Query(result))
    }
}

pub struct Path<T>(pub T);

pub trait FromRoutingMap {
    type Error:IntoResponse;
    fn from_routing_map<F, Args>(
        request: &Request,
        routing: Arc<RoutingMap<F>>,
    ) -> Result<Self, Self::Error>
    where
        Self: Sized,
        F: HandlerFunction<Args>;
}

impl<T> FromRoutingMap for Path<T>
where T:DeserializeOwned {
    type Error = RoutingError;

    fn from_routing_map<F, Args>(
        request: &Request,
        routing: Arc<RoutingMap<F>>,
    ) -> Result<Self, Self::Error>
    where
        Self: Sized,
        F: HandlerFunction<Args>,
    {
        match routing.get_method_router(&request.request_method()) {
            Some(router) => {
                let matched_route = router.at(request.request_path())?;
                let params = matched_route.params;
                let query_string = params
                .iter()
                .map(|(k, v)| format!("{}={}", k, v))
                .collect::<Vec<String>>()
                .join("&");
            println!("query string {}",query_string);
            let extracted_params: T = serde_urlencoded::from_str(&query_string)?;
                Ok(Path(extracted_params))
            }
            None => Err(RoutingError::NotFound),
        }
    }
}

#[derive(Error, Debug)]
pub enum RoutingError {
    #[error("route handler not found")]
    NotFound,
    #[error("path not found {0}")]
    MatchItError(#[from] matchit::MatchError),
    #[error("failed to deserialize multiple params {0}")]
    SerdeUrlEncodingError(#[from] serde_urlencoded::de::Error)
}

impl<'a, T: Deserialize<'a>> TryFrom<&'a Request> for Path<T> {
    type Error = serde_urlencoded::de::Error;
    fn try_from(request: &'a Request) -> Result<Self, Self::Error> {
        let result: T = serde_urlencoded::from_str(request.request_path())?;
        Ok(Path(result))
    }
}

pub trait IntoResponse {
    fn into_response(self)->SendingResponse;
    
}

impl IntoResponse for serde_json::Error{
    fn into_response(self)->SendingResponse {
        let message=b"json data error";
        let headers=get_common_headers_with_content(message);
        SendingResponse::new(StatusMessage::BadRequest, StatusCode::BadRequest, headers,message.to_vec() )
    }
}

impl IntoResponse for serde_urlencoded::de::Error{
    fn into_response(self)->SendingResponse {
        let message=b"url encoded data error";
        let headers=get_common_headers_with_content(message);
        SendingResponse::new(StatusMessage::BadRequest, StatusCode::BadRequest, headers,message.to_vec() )
    }
}

impl IntoResponse for RoutingError{
    fn into_response(self)->SendingResponse {
        let message=b"Not Found or url encoded error";
        let headers=get_common_headers_with_content(message);
        SendingResponse::new(StatusMessage::BadRequest, StatusCode::BadRequest, headers,message.to_vec() )
    }
}


