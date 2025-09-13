use std::{collections::HashMap, io::{Cursor, Read}};

use crate::parser::http_message_parser::find_field_line_index;


pub enum HeaderParseError {
    OtherError,
    InvalidHeader(String),
    HeadersDone,
    NotEnoughBytes,
}
#[derive(Default)]
pub struct HeaderParser{
    headers: HashMap<String, String>,
    trailer_headers: HashMap<String, String>,
}
impl HeaderParser{
    pub fn parse_header(&mut self,data:&[u8]) -> Result<usize, HeaderParseError> {
        if data.starts_with(b"\r\n") {
            return Err(HeaderParseError::HeadersDone);
        }
        let next_field_line_index = find_field_line_index(data).unwrap_or(0);
        let mut cursor = Cursor::new(&data[..next_field_line_index - 2]);
        let mut header_line_str = String::new();
        cursor
            .read_to_string(&mut header_line_str)
            .map_err(|_| HeaderParseError::OtherError)?;
        let parsed_line = parse_header(&header_line_str)?;
        let (key, value) = parsed_line;
        self.set_headers(key, value);
        Ok(next_field_line_index)
    }
    pub fn parse_trailer_header(&mut self,headers_part:&[u8]) -> Result<usize, HeaderParseError> {
        if headers_part.starts_with(b"\r\n") {
            return Err(HeaderParseError::HeadersDone);
        }
        let next_field_line_index =
            find_field_line_index(headers_part).ok_or(HeaderParseError::NotEnoughBytes)?;
        let mut cursor = Cursor::new(&headers_part[..next_field_line_index - 2]);
        let mut header_line_str = String::new();
        cursor
            .read_to_string(&mut header_line_str)
            .map_err(|_| HeaderParseError::OtherError)?;
        let parsed_line = parse_header(&header_line_str)?;
        let (key, value) = parsed_line;
        self.set_trailer_headers(key, value);
        Ok(next_field_line_index)
    }
    fn set_headers(&mut self, key: String, value: String) {
        self.headers
            .entry(key)
            .and_modify(|existing| {
                existing.push(','); // HTTP header values separated by comma-space
                existing.push_str(&value);
            })
            .or_insert(value);
    }
    fn set_trailer_headers(&mut self, key: String, value: String) {
        self.trailer_headers
            .entry(key)
            .and_modify(|existing| {
                existing.push(','); // HTTP header values separated by comma-space
                existing.push_str(&value);
            })
            .or_insert(value);
    }
    pub fn header(&self, key: &str) -> Option<&String> {
        self.headers.get(key)
    }
    pub fn get_headers(self)->HashMap<String,String>{
        self.headers
    }
    pub fn get_headers_ref(&self)->&HashMap<String,String>{
        &self.headers
    }
}


pub fn parse_header(header_field: &str) -> Result<(String, String), HeaderParseError> {
    let broken_parts: Vec<_> = header_field.split(':').collect();

    let key = broken_parts.first().ok_or(HeaderParseError::InvalidHeader(
        "header could not be parsed".to_string(),
    ))?;
    if key.ends_with(' ') {
        return Err(HeaderParseError::InvalidHeader(format!(
            "the key ``{}`` has a space between the field name and colon",
            key
        )));
    }
    if !is_valid_field_name(key) {
        return Err(HeaderParseError::InvalidHeader(format!(
            "the key ``{}`` contains invalid characters",
            key
        )));
    }
    let value = broken_parts[1..].join(":");
    Ok((
        key.to_lowercase().trim().to_string(),
        value.trim().to_string(),
    ))
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
