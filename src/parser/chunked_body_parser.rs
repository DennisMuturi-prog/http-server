use std::io::{Cursor, Read};

use crate::parser::http_message_parser::{find_field_line_index, ParseError};

#[derive(Default)]
enum BodyChunkPart {
    #[default]
    DataSizePart,
    DataContentPart,
}
#[derive(Default)]
pub struct BodyParser{
    body: Vec<u8>,
    body_chunk_part: BodyChunkPart,
    bytes_to_retrieve: usize,
}

impl BodyParser{
    pub fn parse_body(&mut self,data:&[u8])->Result<usize, ParseError> {
        match self.body_chunk_part{
            BodyChunkPart::DataSizePart => {
                self.parse_chunked_body_size(data)
            },
            BodyChunkPart::DataContentPart => {
                self.parse_chunked_body_content(data)
            },
        }

    }
    fn parse_chunked_body_size(&mut self,data:&[u8]) -> Result<usize, ParseError> {
        let next_body_data_index = match find_field_line_index(data) {
            Some(index) => index,
            None => {
                return Err(ParseError::NotEnoughBytes);
            }
        };
        let mut body_chunk_size_str = String::new();
        let mut cursor = Cursor::new(&data[..next_body_data_index - 2]);

        cursor
            .read_to_string(&mut body_chunk_size_str)
            .map_err(|_| ParseError::OtherError("error in reading string to cursor".to_string()))?;

        println!("parsed string is {body_chunk_size_str}");
        let bytes_to_be_retrieved =
            usize::from_str_radix(&body_chunk_size_str, 16).map_err(|_| {
                ParseError::OtherError("error in parsing from hexadecimal string".to_string())
            })?;
        if bytes_to_be_retrieved == 0 {
            return Err(ParseError::HeadersDone);
        }
        self.bytes_to_retrieve = bytes_to_be_retrieved;
        self.set_body_chunk_part();
        Ok(next_body_data_index)
    }
    fn parse_chunked_body_content(&mut self,data:&[u8]) -> Result<usize, ParseError> {
        let next_body_data_size_index = match find_field_line_index(data) {
            Some(index) => index,
            None => {
                return Err(ParseError::NotEnoughBytes);
            }
        };
        let mut body_chunk_size_str = String::new();
        let mut cursor = Cursor::new(&data[..next_body_data_size_index - 2]);

        cursor
            .read_to_string(&mut body_chunk_size_str)
            .map_err(|_| ParseError::OtherError("error in reading string to cursor".to_string()))?;

        println!("parsed string is {body_chunk_size_str}");
        println!("parsed index is {next_body_data_size_index}");

        self.add_chunk_to_body(data)
            .map_err(|err| ParseError::OtherError(err.to_owned()))?;
        self.set_body_chunk_part();
        Ok(next_body_data_size_index)
    }
    fn set_body_chunk_part(&mut self) {
        match self.body_chunk_part {
            BodyChunkPart::DataSizePart => self.body_chunk_part = BodyChunkPart::DataContentPart,
            BodyChunkPart::DataContentPart => self.body_chunk_part = BodyChunkPart::DataSizePart,
        }
    }
    fn add_chunk_to_body(&mut self,data:&[u8]) -> Result<(), &str> {
        match data.get(..self.bytes_to_retrieve){
            Some(chunk) => {
                self.body.extend_from_slice(chunk);
                Ok(())
            },
            None => {
                Err("wrong transfer chunk encoding")
            },
        }
    }
    pub fn add_to_body(&mut self,data:&[u8]) {
        self.body.extend_from_slice(data);
    }
    pub fn get_body(self)->Vec<u8>{
        self.body
    }

}