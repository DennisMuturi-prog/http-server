

use serde::de::DeserializeOwned;
use thiserror::Error;

use crate::{
    parser::http_message_parser::Request,
    response::
        IntoResponse
    ,
};

pub struct Json<T>(pub T);

pub trait FromRequest {
    type Error: IntoResponse;
    fn from_request(request: &Request) -> Result<Self, Self::Error>
    where
        Self: std::marker::Sized;
}

pub trait FromRequestBody {
    type Error: IntoResponse;
    fn from_request_body(request: &Request) -> Result<Self, Self::Error>
    where
        Self: std::marker::Sized;
}

impl<T> FromRequestBody for Json<T>
where
    T: DeserializeOwned,
{
    type Error = BodyContentError;
    fn from_request_body(request: &Request) -> Result<Self, Self::Error> {
        match request.header("content-type") {
            Some(header) => {
                if header != "application/json" {
                    return Err(BodyContentError::ContentTypeMisMatch);
                }
            }
            None => return Err(BodyContentError::ContentTypeMisMatch),
        };
        let result: T = serde_json::from_slice(request.body())?;
        Ok(Json(result))
    }
}
pub struct Form<T>(pub T);
impl<T> FromRequestBody for Form<T>
where
    T: DeserializeOwned,
{
    type Error = BodyContentError;
    fn from_request_body(request: &Request) -> Result<Self, Self::Error> {
        match request.header("content-type") {
            Some(header) => {
                if header != "application/x-www-form-urlencoded" {
                    println!("header is {}",header);
                    return Err(BodyContentError::ContentTypeMisMatch);
                }
            }
            None => return Err(BodyContentError::ContentTypeMisMatch),
        };
        let result: T = serde_urlencoded::from_bytes(request.body())?;
        Ok(Form(result))
    }
}
#[derive(Error, Debug)]
pub enum BodyContentError {
    #[error("content type header mismatch")]
    ContentTypeMisMatch,
    #[error("json serialization error: {0}")]
    JsonSerializationError(#[from] serde_json::Error),
    #[error("route handler not found {0}")]
    UrlEncodedFormSerialization(#[from] serde_urlencoded::de::Error),
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

impl<T> FromRequest for Path<T>
where
    T: DeserializeOwned,
{
    type Error = RoutingError;

    fn from_request(
        request: &Request
    ) -> Result<Self, Self::Error>
    where
        Self: Sized,
    {
        match request.routing().get_method_router(&request.request_method()) {
            Some(router) => {
                let matched_route = router.at(request.request_path())?;
                let params = matched_route.params;
                let query_string = params
                    .iter()
                    .map(|(k, v)| format!("{}={}", k, v))
                    .collect::<Vec<String>>()
                    .join("&");
                println!("query string {}", query_string);
                let extracted_params: T = serde_urlencoded::from_str(&query_string)?;
                Ok(Path(extracted_params))
            }
            None => Err(RoutingError::NotFound),
        }
    }
}

impl<T> FromRequestBody for T
where T:FromRequest{
    type Error=<Self as FromRequest>::Error;

    fn from_request_body(request: &Request) -> Result<Self, Self::Error>
    where
        Self: std::marker::Sized {
        Self::from_request(request)
    }
}


#[derive(Error, Debug)]
pub enum RoutingError {
    #[error("route handler not found")]
    NotFound,
    #[error("path not found {0}")]
    MatchItError(#[from] matchit::MatchError),
    #[error("failed to deserialize multiple params {0}")]
    SerdeUrlEncodingError(#[from] serde_urlencoded::de::Error),
}

