use std::io;

use bytes::Bytes;

use crate::header::FlvHeader;

pub struct FlvParser;

impl FlvParser {
    pub fn parse_header(reader: &mut io::Cursor<Bytes>) -> io::Result<FlvHeader> {
        FlvHeader::parse(reader)
    }
}
