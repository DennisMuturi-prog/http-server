use std::io::{Cursor, Read};

pub enum ParseError {
    NotEnoughBytes,
    ResponseLinePartsMissing,
    OtherError,
    MissingHttpVersion,
    InvalidHeader(String),
    HeadersDone,
}

pub struct NotEnoughBytes;

pub enum HeaderParseError {
    OtherError,
    InvalidHeader(String),
    HeadersDone,
}
pub trait HttpMessage {
    type FirstLineParseError;
    fn parse_front(&mut self) -> Result<usize, NotEnoughBytes> {
        let first_index_of_body =
            find_payload_index(self.get_data()).ok_or(NotEnoughBytes)?;
        self.set_body_cursor(first_index_of_body);
        Ok(first_index_of_body)
    }
    fn parse_headers(&mut self) -> Result<usize, HeaderParseError> {
        if self.get_current_position() >= self.get_body_cursor() - 2 {
            self.set_current_position(2);
            return Err(HeaderParseError::HeadersDone);
        }
        let headers_part = self.get_current_part();
        let next_field_line_index = find_field_line_index(headers_part).unwrap_or(0);
        let mut cursor = Cursor::new(&headers_part[..next_field_line_index - 2]);
        let mut header_line_str = String::new();
        cursor
            .read_to_string(&mut header_line_str)
            .map_err(|_| HeaderParseError::OtherError)?;
        let parsed_line = parse_headers(&header_line_str)?;
        let (key, value) = parsed_line;
        self.set_current_position(next_field_line_index);
        self.set_headers(key, value);
        Ok(next_field_line_index)
    }
    fn parse_chunked_body(&mut self) -> Result<usize, ParseError> {
        let body=&self.get_current_part();
        let next_body_data_index = match find_field_line_index(body){
            Some(index) => index,
            None => {
                return Err(ParseError::NotEnoughBytes);
            },
        };
        let mut body_chunk_size_str = String::new();
        let mut cursor = Cursor::new(&body[..next_body_data_index-2]);

        cursor
            .read_to_string(&mut body_chunk_size_str)
            .map_err(|_| ParseError::OtherError)?;
        let bytes_to_be_retrieved = usize::from_str_radix(&body_chunk_size_str, 16).unwrap();
        if bytes_to_be_retrieved == 0 {
            return Err(ParseError::HeadersDone);
        }
        self.set_current_position(next_body_data_index);
        self.set_bytes_to_retrieve(bytes_to_be_retrieved); 
        self.set_data_content_part();
        
        Ok(next_body_data_index)
    }
    fn add_chunked_body_content(&mut self) -> Result<usize, ParseError> {
        let body=&self.get_current_part();
        let next_body_data_size_index = match find_field_line_index(body){
            Some(index) => index,
            None => {
                return Err(ParseError::NotEnoughBytes);
            },
        };
        let mut body_chunk_data_str = String::new();
        let mut cursor = Cursor::new(&body[..next_body_data_size_index-2]);

        cursor
            .read_to_string(&mut body_chunk_data_str)
            .map_err(|_| ParseError::OtherError)?;
        println!("body chunk {}",body_chunk_data_str);
        self.set_current_position(next_body_data_size_index);
        self.set_data_content_part();
        Ok(2)
    }
    fn parse_first_line(&mut self)->Result<usize,Self::FirstLineParseError>;

    fn set_bytes_to_retrieve(&mut self,bytes_size:usize);
    fn set_data_content_part(&mut self);
    // fn get_left_over_first_part(&self)->&[u8];

    

    fn get_data(&self) -> &[u8];
    fn free_parsed_data(&mut self);
    fn get_current_part(&self) -> &[u8];
    fn get_current_position(&self) -> usize;
    fn set_current_position(&mut self, index: usize);
    fn get_body_cursor(&self) -> usize;
    fn set_body_cursor(&mut self, index: usize);
    // fn set_body_position(&mut self, index: usize);
    // fn get_body(&self)->&[u8];
    fn set_headers(&mut self, key: String, value: String);
    // fn add_to_body(&mut self,buf:&[u8]);
    fn add_to_data(&mut self,buf:&[u8]);
    fn get_header(&self,key:&str)->Option<&String>;
    fn get_body_len(&self)->usize;
    fn get_data_content_part_state(&self)->bool;

}

fn parse_headers(header_field: &str) -> Result<(String, String), HeaderParseError> {
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
fn find_payload_index(buffer: &[u8]) -> Option<usize> {
    buffer
        .windows(4)
        .position(|w| matches!(w, b"\r\n\r\n"))
        .map(|ix| ix + 4)
}

fn find_field_line_index(buffer: &[u8]) -> Option<usize> {
    buffer
        .windows(2)
        .position(|w| matches!(w, b"\r\n"))
        .map(|ix| ix + 2)
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
