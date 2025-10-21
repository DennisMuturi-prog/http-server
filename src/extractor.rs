use std::sync::Arc;

use serde::{Deserialize, Serialize, de::DeserializeOwned};
use thiserror::Error;

use crate::{
    parser::http_message_parser::Request,
    response::{
        ContentType, SendingResponse, StatusCode, StatusMessage,
        get_common_headers_with_content_type_header,
    },
    routing::{HandlerFunction, RoutingMap},
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

pub trait FromRoutingMap {
    type Error: IntoResponse;
    fn from_routing_map<F, Args>(
        request: &Request,
        routing: Arc<RoutingMap<F>>,
    ) -> Result<Self, Self::Error>
    where
        Self: Sized,
        F: HandlerFunction<Args>;
}

impl<T> FromRoutingMap for Path<T>
where
    T: DeserializeOwned,
{
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
                println!("query string {}", query_string);
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
    SerdeUrlEncodingError(#[from] serde_urlencoded::de::Error),
}
pub trait IntoResponse {
    fn into_response(self) -> SendingResponse;
}

impl IntoResponse for serde_json::Error {
    fn into_response(self) -> SendingResponse {
        let message = b"json data error";
        let headers = get_common_headers_with_content_type_header(message, ContentType::TextPlain);
        SendingResponse::new(
            StatusMessage::BadRequest,
            StatusCode::BadRequest,
            headers,
            message.to_vec(),
        )
    }
}

impl IntoResponse for serde_urlencoded::de::Error {
    fn into_response(self) -> SendingResponse {
        let message = b"url encoded data error";
        let headers = get_common_headers_with_content_type_header(message, ContentType::TextPlain);
        SendingResponse::new(
            StatusMessage::BadRequest,
            StatusCode::BadRequest,
            headers,
            message.to_vec(),
        )
    }
}

impl IntoResponse for serde_urlencoded::ser::Error {
    fn into_response(self) -> SendingResponse {
        let message = b"server serialization error";
        let headers = get_common_headers_with_content_type_header(message, ContentType::TextPlain);

        SendingResponse::new(
            StatusMessage::InternalServerError,
            StatusCode::InternalServerError,
            headers,
            message.to_vec(),
        )
    }
}

impl IntoResponse for RoutingError {
    fn into_response(self) -> SendingResponse {
        let message = b"Not Found or url encoded error";
        let headers = get_common_headers_with_content_type_header(message, ContentType::TextPlain);
        SendingResponse::new(
            StatusMessage::BadRequest,
            StatusCode::BadRequest,
            headers,
            message.to_vec(),
        )
    }
}

impl IntoResponse for BodyContentError {
    fn into_response(self) -> SendingResponse {
        match self {
            BodyContentError::ContentTypeMisMatch => {
                let message = b"content type mismatch";
                let headers =
                    get_common_headers_with_content_type_header(message, ContentType::TextPlain);
                SendingResponse::new(
                    StatusMessage::BadRequest,
                    StatusCode::BadRequest,
                    headers,
                    message.to_vec(),
                )
            }
            BodyContentError::JsonSerializationError(_) => {
                let message = b"content type mismatch";

                let headers =
                    get_common_headers_with_content_type_header(message, ContentType::TextPlain);

                SendingResponse::new(
                    StatusMessage::BadRequest,
                    StatusCode::BadRequest,
                    headers,
                    message.to_vec(),
                )
            }
            BodyContentError::UrlEncodedFormSerialization(_) => {
                let message = b"content type mismatch";
                let headers =
                    get_common_headers_with_content_type_header(message, ContentType::TextPlain);
                SendingResponse::new(
                    StatusMessage::BadRequest,
                    StatusCode::BadRequest,
                    headers,
                    message.to_vec(),
                )
            }
        }
    }
}

impl IntoResponse for SendingResponse {
    fn into_response(self) -> SendingResponse {
        self
    }
}

impl<T> IntoResponse for Form<T>
where
    T: Serialize,
{
    fn into_response(self) -> SendingResponse {
        let result = match serde_urlencoded::to_string(self.0) {
            Ok(val) => val,
            Err(err) => return err.into_response(),
        };

        let headers = get_common_headers_with_content_type_header(result.as_bytes(),ContentType::ApplicationUrlEncoded);
        SendingResponse::new(
            StatusMessage::Ok,
            StatusCode::Ok,
            headers,
            result.as_bytes().to_vec(),
        )
    }
}


impl<T> IntoResponse for Json<T>
where
    T: Serialize,
{
    fn into_response(self) -> SendingResponse {
        let result = match serde_json::to_string(&self.0) {
            Ok(val) => val,
            Err(err) => return err.into_response(),
        };

        let headers = get_common_headers_with_content_type_header(result.as_bytes(),ContentType::ApplicationJson);
        SendingResponse::new(
            StatusMessage::Ok,
            StatusCode::Ok,
            headers,
            result.as_bytes().to_vec(),
        )
    }
}
