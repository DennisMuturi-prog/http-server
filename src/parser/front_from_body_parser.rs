pub struct NotEnoughBytes;
pub fn parse_front(data:&[u8]) -> Result<usize, NotEnoughBytes> {
        let first_index_of_body = find_payload_index(data).ok_or(NotEnoughBytes)?;
        Ok(first_index_of_body)
}

pub fn find_payload_index(buffer: &[u8]) -> Option<usize> {
    buffer
        .windows(4)
        .position(|w| matches!(w, b"\r\n\r\n"))
        .map(|ix| ix + 4)
}