use std::{collections::HashMap, error::Error, io::{BufRead, Read}, net::TcpStream};

pub fn request_from_reader(stream: &mut TcpStream) -> Result<Request, Box<dyn Error>> {
    let mut buf = [0; 1024];
    let mut my_bytes = Vec::<u8>::with_capacity(120);
    let mut no_of_bytes_parsed = 0;
    let mut request = Request::default();
    let mut request_line_parsed = 0;
    let mut n = stream.read(&mut buf)?;
    my_bytes.append(&mut buf[..n].to_vec());
    loop {
        if request_line_parsed == 0 {
            match request.parse(&my_bytes) {
                Ok(no) => {
                    request_line_parsed = 1;
                    no_of_bytes_parsed += no + 2;
                }
                Err(err) => match err {
                    ParseError::NotEnoughBytes => {
                        n = stream.read(&mut buf)?;
                        my_bytes.append(&mut buf[..n].to_vec());
                    }
                    ParseError::OtherError => {
                        return Err("another error".into());
                    }
                    ParseError::InvalidHeader(cause) => {
                        return Err(cause.into());
                    }
                    ParseError::RequestLinePartsMissing => {
                        return Err("parts of request line missing and could not be parsed".into());
                    }
                    ParseError::InvalidHttpMethod => {
                        return Err("invalid http method".into());
                    }
                    ParseError::MissingHttpVersion => {
                        return Err("the version of http could not be parsed".into());
                    }
                    ParseError::HeadersDone => {
                        return Err("the version of http could not be parsed".into());
                    }
                },
            };
        } else if request_line_parsed == 1 {
            match request.parse_request_headers(&my_bytes[no_of_bytes_parsed..]) {
                Ok(no) => {
                    no_of_bytes_parsed += no + 2;
                }
                Err(err) => match err {
                    ParseError::NotEnoughBytes => {
                        n = stream.read(&mut buf)?;
                        my_bytes.append(&mut buf[..n].to_vec());
                    }
                    ParseError::HeadersDone => {
                        no_of_bytes_parsed += 2;
                        request.add_bytes_to_body(&my_bytes[no_of_bytes_parsed..]);
                        request_line_parsed = 2;
                        let content_length = match request.headers.get("content-length") {
                            Some(content_len) => content_len,
                            None => {
                                if request.request_line.method=="OPTIONS"{
                                    return Ok(request);

                                }else{
                                    return Ok(request);

                                }
                            }
                        }
                        .parse::<usize>()?;
                        if request.body.len() >= content_length {
                            return Ok(request);
                        }
                    }
                    ParseError::OtherError => {
                        return Err("another error".into());
                    }
                    ParseError::InvalidHeader(cause) => {
                        return Err(cause.into());
                    }
                    ParseError::RequestLinePartsMissing => {
                        return Err("parts of request missing and could not be parsed".into());
                    }
                    ParseError::InvalidHttpMethod => {
                        return Err("invalid http method".into());
                    }
                    ParseError::MissingHttpVersion => {
                        return Err("the version of http could not be parsed".into());
                    }
                },
            };
        } else {
            n = stream.read(&mut buf)?;
            request.add_bytes_to_body(&buf[..n]);
            let content_length = request
                .headers
                .get("content-length")
                .ok_or("error occurred")?
                .parse::<usize>()?;
            if request.body.len() >= content_length {
                return Ok(request);
            }
        }
    }
}

#[derive(Debug)]
enum ParseError {
    NotEnoughBytes,
    RequestLinePartsMissing,
    InvalidHttpMethod,
    OtherError,
    MissingHttpVersion,
    InvalidHeader(String),
    HeadersDone,
}

#[derive(Debug, Default)]
pub struct Request {
    request_line: RequestLine,
    headers: HashMap<String, String>,
    body: Vec<u8>,
}
#[derive(Debug, Default)]
struct RequestLine {
    http_version: String,
    request_target: String,
    method: String,
}
impl Request {
    fn parse(&mut self, request_line: &Vec<u8>) -> Result<usize, ParseError> {
        if request_line.is_empty(){
            println!("request line empty");
            return Err(ParseError::OtherError);
        }
        let lines: Vec<_> = request_line.lines().collect();
        // println!("lines:{:?}",request_line);
        if lines.len() == 1 {
            return Err(ParseError::NotEnoughBytes);
        }
        let not_parsed_line = lines[0].as_ref().map_err(|_| ParseError::OtherError)?;
        let parsed_line =
            parse_request_line(not_parsed_line).map_err(|_| ParseError::OtherError)?;
        self.request_line = parsed_line;
        Ok(not_parsed_line.len())
    }
    fn parse_request_headers(&mut self, request_line: &[u8]) -> Result<usize, ParseError> {
        let lines: Vec<_> = request_line.lines().collect();
        let not_parsed_line = lines[0].as_ref().map_err(|_| ParseError::OtherError)?;
        if lines.len() == 1 && !not_parsed_line.is_empty() {
            return Err(ParseError::NotEnoughBytes);
        }
        if not_parsed_line.is_empty() {
            return Err(ParseError::HeadersDone);
        }
        let (key, value) = parse_headers(not_parsed_line)?;
        self.add_header(key, value);
        Ok(not_parsed_line.len())
    }
    fn add_header(&mut self, key: String, value: String) {
        self.headers
            .entry(key)
            .and_modify(|existing| {
                existing.push_str(","); // HTTP header values separated by comma-space
                existing.push_str(&value);
            })
            .or_insert(value);
    }
    fn add_bytes_to_body(&mut self, buf: &[u8]) {
        self.body.append(&mut buf.to_vec())
    }
    pub fn get_request_path(&self)->&str{
        &self.request_line.request_target

    }
    pub fn get_request_method(&self)->&str{
        &self.request_line.method

    }
}

fn parse_headers(header_field: &str) -> Result<(String, String), ParseError> {
    let broken_parts: Vec<_> = header_field.split(':').collect();

    let key = broken_parts.get(0).ok_or(ParseError::InvalidHeader(
        "header could not be parsed".to_string(),
    ))?;
    if key.ends_with(' ') {
        return Err(ParseError::InvalidHeader(format!(
            "the key ``{}`` has a space between the field name and colon",
            key.to_string()
        )));
    }
    if !is_valid_field_name(key) {
        return Err(ParseError::InvalidHeader(format!(
            "the key ``{}`` contains invalid characters",
            key.to_string()
        )));
    }
    let value = broken_parts[1..].join(":");
    Ok((
        key.to_lowercase().trim().to_string(),
        value.trim().to_string(),
    ))
}

fn parse_request_line(request_line: &str) -> Result<RequestLine, ParseError> {
    let http_verbs = ["GET", "POST", "PATCH", "DELETE", "PUT", "OPTIONS"];
    let broken_string = request_line.split(' ').collect::<Vec<&str>>();
    if broken_string.len() < 3 {
        return Err(ParseError::RequestLinePartsMissing);
    }
    let mut http_verb = String::new();
    if http_verbs.contains(&broken_string[0]) {
        http_verb.push_str(&broken_string[0]);
    } else {
        return Err(ParseError::InvalidHttpMethod);
    }
    let http_version_parts: Vec<_> = broken_string[2].split('/').collect();
    let http_version = match http_version_parts.get(1) {
        Some(version) => version,
        None => {
            return Err(ParseError::MissingHttpVersion);
        }
    };
    Ok(RequestLine {
        http_version: http_version.to_string(),
        method: http_verb,
        request_target: broken_string[1].to_string(),
    })
}

fn is_valid_field_name(s: &str) -> bool {
    s.chars().all(|c| {
        c.is_ascii_alphanumeric()
            || matches!(
                c,
                '!' | '#'
                    | '$'
                    | '%'
                    | '&'
                    | '\''
                    | '*'
                    | '+'
                    | '-'
                    | '.'
                    | '^'
                    | '_'
                    | '`'
                    | '|'
                    | '~'
            )
    })
}