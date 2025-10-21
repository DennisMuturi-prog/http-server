use std::{collections::HashMap, io::{Result as IoResult, Write}};

use crate::parser::first_line_parser::{ResponseLine};


pub struct SendingResponse{
    status_message:StatusMessage,
    status_code:StatusCode,
    headers:HashMap<String,String>,
    body:Vec<u8>
}

impl SendingResponse{
    pub fn new(status_message:StatusMessage,status_code:StatusCode,headers:HashMap<String,String>,body:Vec<u8>)->Self{
        Self { status_message, status_code, headers, body}


    }
    pub fn headers(&self)->&HashMap<String,String>{
        &self.headers
    }
    pub fn body(&self)->&[u8]{
        &self.body
    }
    pub fn status_code(&self)->&StatusCode{
        &self.status_code
    }
}

pub enum StatusMessage {
    // 1xx Informational
    Continue = 100,
    SwitchingProtocols = 101,
    Processing = 102,
    EarlyHints = 103,

    // 2xx Success
    Ok = 200,
    Created = 201,
    Accepted = 202,
    NonAuthoritativeInformation = 203,
    NoContent = 204,
    ResetContent = 205,
    PartialContent = 206,
    MultiStatus = 207,
    AlreadyReported = 208,
    ImUsed = 226,

    // 3xx Redirection
    MultipleChoices = 300,
    MovedPermanently = 301,
    Found = 302,
    SeeOther = 303,
    NotModified = 304,
    UseProxy = 305,
    TemporaryRedirect = 307,
    PermanentRedirect = 308,

    // 4xx Client Errors
    BadRequest = 400,
    Unauthorized = 401,
    PaymentRequired = 402,
    Forbidden = 403,
    NotFound = 404,
    MethodNotAllowed = 405,
    NotAcceptable = 406,
    ProxyAuthenticationRequired = 407,
    RequestTimeout = 408,
    Conflict = 409,
    Gone = 410,
    LengthRequired = 411,
    PreconditionFailed = 412,
    PayloadTooLarge = 413,
    UriTooLong = 414,
    UnsupportedMediaType = 415,
    RangeNotSatisfiable = 416,
    ExpectationFailed = 417,
    ImATeapot = 418,
    MisdirectedRequest = 421,
    UnprocessableEntity = 422,
    Locked = 423,
    FailedDependency = 424,
    TooEarly = 425,
    UpgradeRequired = 426,
    PreconditionRequired = 428,
    TooManyRequests = 429,
    RequestHeaderFieldsTooLarge = 431,
    UnavailableForLegalReasons = 451,

    // 5xx Server Errors
    InternalServerError = 500,
    NotImplemented = 501,
    BadGateway = 502,
    ServiceUnavailable = 503,
    GatewayTimeout = 504,
    HttpVersionNotSupported = 505,
    VariantAlsoNegotiates = 506,
    InsufficientStorage = 507,
    LoopDetected = 508,
    NotExtended = 510,
    NetworkAuthenticationRequired = 511,
}


pub enum StatusCode {
    Ok,
    BadRequest,
    InternalServerError,
    NotFound,
    MethodNotAllowed
}

pub enum ContentType{
    ApplicationJson,
    ApplicationUrlEncoded,
    TextPlain,
    ImageJpeg,
    TextHtml,
}

pub fn write_status_line<T: Write>(stream_writer: &mut T, status: StatusCode) -> IoResult<()> {
    let mut status = match status {
        StatusCode::Ok => String::from("HTTP/1.1 200 OK"),
        StatusCode::BadRequest => String::from("HTTP/1.1 400 Bad Request"),
        StatusCode::InternalServerError => String::from("HTTP/1.1 500 Internal Server Error"),
        StatusCode::NotFound=> String::from("HTTP/1.1 404 Not Found"),
        StatusCode::MethodNotAllowed=>String::from("HTTP/1.1 405 Method Not Allowed")
    };
    status.push_str("\r\n");
    stream_writer.write_all(status.as_bytes())?;
    Ok(())
}
pub fn write_response_status_line<T: Write>(stream_writer: &mut T, status: &StatusCode) -> IoResult<()> {
    let mut status = match status {
        StatusCode::Ok => String::from("HTTP/1.1 200 OK"),
        StatusCode::BadRequest => String::from("HTTP/1.1 400 Bad Request"),
        StatusCode::InternalServerError => String::from("HTTP/1.1 500 Internal Server Error"),
        StatusCode::NotFound=> String::from("HTTP/1.1 404 Not Found"),
        StatusCode::MethodNotAllowed=>String::from("HTTP/1.1 405 Method Not Allowed")
    };
    status.push_str("\r\n");
    stream_writer.write_all(status.as_bytes())?;
    Ok(())
}



pub fn write_proxied_response_status_line<T: Write>(
    stream_writer: &mut T,
    response: &ResponseLine,
) -> IoResult<()> {
    let status_line = format!(
        "HTTP/1.1 {} {}\r\n",
        response.status_code(),
        response.status_message()
    );
    stream_writer.write_all(status_line.as_bytes())?;
    Ok(())
}

pub fn get_preflight_headers() -> HashMap<&'static str, &'static str> {
    HashMap::from([
        ("Content-Length", "0"),
        ("Content-Type", "text/plain"),
        ("Access-Control-Allow-Origin", "https://hoppscotch.io"),
        ("Access-Control-Allow-Methods", "*"),
        ("Access-Control-Allow-Headers", "*"),
        ("Connection", "close"),
    ])
}

pub fn get_common_headers() -> HashMap<&'static str, &'static str> {
    HashMap::from([
        ("Access-Control-Allow-Origin", "https://hoppscotch.io"),
        ("Content-Length","0"),
        ("Connection", "close"),
    ])
}



pub fn get_common_headers_with_content_type_header(body:&[u8],content_type:ContentType) -> HashMap<String, String> {
    let content_type=match content_type{
        ContentType::ApplicationJson => "application/json",
        ContentType::ApplicationUrlEncoded => "application/x-www-form-urlencoded",
        ContentType::TextPlain => "text/plain",
        ContentType::ImageJpeg => "image/jpeg",
        ContentType::TextHtml => "text/html",
    };
    let body_length=body.len();
    HashMap::from([
        ("Access-Control-Allow-Origin".to_string(), "https://hoppscotch.io".to_string()),
        ("Content-Length".to_string(),body_length.to_string()),
        ("Connection".to_string(), "close".to_string()),
        ("Content-Type".to_string(), content_type.to_string()),
    ])
}

pub fn write_headers<T: Write>(
    stream_writer: &mut T,
    headers: HashMap<&str, &str>,
) -> IoResult<()> {
    let mut headers_response = String::new();
    for (key, value) in headers {
        headers_response.push_str(key);
        headers_response.push_str(": ");
        headers_response.push_str(value);
        headers_response.push_str("\r\n");
    }
    headers_response.push_str("\r\n");
    stream_writer.write_all(headers_response.as_bytes())?;
    Ok(())
}

pub fn write_response_headers<T: Write>(
    stream_writer: &mut T,
    headers: &HashMap<String, String>,
) -> IoResult<()> {
    let mut headers_response = String::new();
    for (key, value) in headers {
        headers_response.push_str(key);
        headers_response.push_str(": ");
        headers_response.push_str(value);
        headers_response.push_str("\r\n");
    }
    headers_response.push_str("\r\n");
    stream_writer.write_all(headers_response.as_bytes())?;
    Ok(())
}




