use std::{error::Error, io::Read, net::TcpStream};

use crate::{http_message_parser::{HeaderParseError, HttpMessage, ParseError}, response_parser::{FirstLineParseError, ResponseParser}};

pub fn response_from_reader_general(stream: &mut TcpStream) -> Result<ResponseParser, Box<dyn Error>> {
    let mut buf = [0; 1024];
    let mut response = ResponseParser::new(stream);
    let mut response_line_parsed = 0;
    let mut n = stream.read(&mut buf)?;
    response.add_to_data(&buf[..n]);
    loop {
        if response_line_parsed==0{
            match response.parse_front() {
                Ok(_) => {
                    response_line_parsed = 1;
                }
                Err(_) =>{
                    n = stream.read(&mut buf)?;
                    response.add_to_data(&buf[..n]);

                }
            };

        }
        else if response_line_parsed == 1 {
            match response.parse_first_line() {
                Ok(_) => {
                    response_line_parsed = 2;
                }
                Err(err) => match err {
                    FirstLineParseError::OtherError => {
                        return Err("another error".into());
                    }
                    
                    FirstLineParseError::ResponseLinePartsMissing => {
                        return Err("parts of response line missing and could not be parsed".into());
                    }
                    FirstLineParseError::MissingHttpVersion => {
                        return Err("the version of http could not be parsed".into());
                    }
                },
            };
        } else if response_line_parsed == 2 {
            match response.parse_headers() {
                Ok(_) => {
                }
                Err(err) => match err {
                    
                    HeaderParseError::HeadersDone => {
                        response_line_parsed = 3;
                        let content_length = match response.get_header("content-length") {
                            Some(content_len) => content_len,
                            None => {
                                let transfer_encoding_chunked =
                                    match response.get_header("transfer-encoding") {
                                        Some(chunking) => chunking,
                                        None => {
                                            return Ok(response);
                                        }
                                    };
                                if transfer_encoding_chunked == "chunked" {
                                    response_line_parsed = 4;
                                } else {
                                    return Ok(response);
                                }
                                continue;
                            }
                        }
                        .parse::<usize>()?;
                        if response.get_body_len() >= content_length {
                            return Ok(response);
                        }
                    }
                    HeaderParseError::OtherError => {
                        return Err("another error".into());
                    }
                    HeaderParseError::InvalidHeader(cause) => {
                        return Err(cause.into());
                    }
                    
                },
            };
        } else if response_line_parsed == 3 {
            n = stream.read(&mut buf)?;
            response.add_to_data(&buf[..n]);
            let content_length = response
                .get_header("content-length")
                .ok_or("error occurred")?
                .parse::<usize>()?;
            if response.get_body_len() >= content_length {
                return Ok(response);
            }
        } else { 
            if response.get_data_content_part_state() {
                match response.add_chunked_body_content() {
                    Ok(_) => {}
                    Err(err) => match err {
                        ParseError::NotEnoughBytes => {
                            n = stream.read(&mut buf)?;
                            response.add_to_data(&buf[..n]);
                        }
                        ParseError::HeadersDone => {
                            return Ok(response);
                        }
                        _ => return Ok(response),
                    },
                }
            } else {
                match response.parse_chunked_body() {
                    Ok(_) => {}
                    Err(err) => match err {
                        ParseError::NotEnoughBytes => {
                            n = stream.read(&mut buf)?;
                            response.add_to_data(&buf[..n]);
                        }
                        ParseError::HeadersDone => {
                            return Ok(response);
                        }
                        _ => return Ok(response),
                    },
                }
            }
        }
    }
}
