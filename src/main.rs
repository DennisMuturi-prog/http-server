use std::{
    collections::HashMap,
    error::Error,
    io::{self, Result as IoResult, prelude::*},
    net::{TcpListener, TcpStream},
    thread,
};

fn main() -> IoResult<()> {
    let listener = TcpListener::bind("127.0.0.1:7878").unwrap();
    loop {
        let (connection, _) = listener.accept().unwrap();
        match request_from_reader(connection) {
            Ok(parsed_request) => {
                println!("parsed Request:{:?}", parsed_request);
                match String::from_utf8(parsed_request.body) {
                    Ok(parsed_body) => {
                        println!("parsed body:{}", parsed_body);
                    }
                    Err(_) => {
                        println!("failed to parse body");
                    }
                };
            }
            Err(err) => {
                println!("err:{:?}", err);
                break;
            }
        }
    }
    // for stream in listener.incoming() {
    //     let stream = stream.unwrap();
    //     thread::spawn(||{

    //     });
    // }
    // let mock_stream = MockStream {
    //     pos: 0,
    //     data: String::from(
    //         "GET /coffee HTTP/1.1\r\nHost: localhost:42069\r\nUser-Agent: curl/7.81.0\r\nAccept: */*\r\n\r\npartial content probably praise the lord 😂 hello world!",
    //     ),
    // };
    // match request_from_reader(mock_stream) {
    //     Ok(parsed_request) => {
    //         println!("parsed Request:{:?}", parsed_request);
    //         match String::from_utf8(parsed_request.body) {
    //             Ok(parsed_body) => {
    //                 println!("parsed body:{}", parsed_body);
    //             }
    //             Err(_) => {
    //                 println!("failed to parse body");
    //             }
    //         };
    //     }
    //     Err(err) => {
    //         println!("err:{:?}", err);
    //     }
    // }

    Ok(())
}

fn request_from_reader(mut stream: TcpStream) -> Result<Request, Box<dyn Error>> {
    let mut buf = [0; 1];
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
                        let content_length = request
                            .headers
                            .get("content-length")
                            .ok_or("error occurred")?
                            .parse::<usize>()?;
                        if request.body.len() >= content_length {
                            let response = "HTTP/1.1 200 OK\r\n\r\n";
                            stream.write_all(response.as_bytes()).unwrap();
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
                let response = "HTTP/1.1 200 OK\r\n\r\n";
                stream.write_all(response.as_bytes()).unwrap();
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
struct Request {
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

struct MockStream {
    data: String,
    pos: usize,
}

impl Read for MockStream {
    fn read(&mut self, buf: &mut [u8]) -> IoResult<usize> {
        let data_len = self.data.len();
        if self.pos >= data_len {
            return Ok(0);
        }
        let data_bytes = self.data.as_bytes();
        let mut offset = buf.len();
        let mut end_index = self.pos + offset;
        if end_index > data_len {
            let overflow = end_index - data_len;
            offset -= overflow;
            end_index = data_len;
        }
        for (index, byte) in data_bytes[self.pos..end_index].iter().enumerate() {
            buf[index] = *byte;
        }
        self.pos += offset;

        Ok(offset)
    }
}
