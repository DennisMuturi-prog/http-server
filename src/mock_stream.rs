use std::io::{self, Read, Result as IoResult, Write};

pub struct MockStream {
    data: String,
    pos: usize,
    received_data:String
}

impl Write for MockStream{
    fn write(&mut self, buf: &[u8]) -> IoResult<usize> {
        let received_string=String::from_utf8(buf.to_vec()).map_err( io::Error::other)?;
        self.received_data.push_str(&received_string);
        Ok(buf.len())

    }

    fn flush(&mut self) -> IoResult<()> {
        todo!()
    }
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