use byteorder::{BigEndian, ReadBytesExt};
use bytes::{Buf, BytesMut};
use std::fs::File;
use std::io::{self, BufReader, Bytes, Cursor, Read};
use std::path::Path;

use crate::file::FlvFile;
use crate::header::FlvHeader;
use crate::tag::{self, FlvTagOwned, FlvUtil};

const BUFFER_SIZE: usize = 4 * 1024; // 8 KB buffer size

pub struct FlvParser;

impl FlvParser {
    pub fn parse_file(file_path: &Path) -> io::Result<u32> {
        let file = File::open(file_path)?;
        let mut reader = BufReader::new(file);
        let mut buffer = BytesMut::with_capacity(4096);

        // Read and parse FLV header
        buffer.resize(9, 0);
        reader.read_exact(&mut buffer)?;
        let mut cursor = Cursor::new(buffer.freeze());
        let header = FlvHeader::parse(&mut cursor)?;
        let mut tags: Vec<FlvTagOwned> = Vec::new();
        let mut tags_count = 0;

        // Create a new buffer for reading tags
        let mut tag_buffer = BytesMut::with_capacity(BUFFER_SIZE);

        loop {
            // Read previous tag size (4 bytes)
            tag_buffer.resize(4, 0);
            match reader.read_exact(&mut tag_buffer) {
                Ok(_) => {
                    // Just ignore the prev tag size for now
                    let _prev_tag_size = (&tag_buffer[..]).get_u32();
                }
                Err(e) if e.kind() == io::ErrorKind::UnexpectedEof => {
                    break;
                }
                Err(e) => return Err(e),
            }

            // Peek at tag header (first 11 bytes) to get the data size
            tag_buffer.resize(11, 0);
            if let Err(e) = reader.read_exact(&mut tag_buffer) {
                if e.kind() == io::ErrorKind::UnexpectedEof {
                    break;
                }
                return Err(e);
            }

            // Get the data size from the first 11 bytes without consuming the buffer
            let data_size = {
                let mut peek = Cursor::new(&tag_buffer[..]);
                peek.advance(1); // Skip tag type byte
                peek.read_u24::<BigEndian>()?
            };

            // Now read the complete tag (header + data)
            // Reset position to beginning of tag
            let total_tag_size = 11 + data_size as usize;

            // Resize the buffer to fit the entire tag
            // We've already read the first 11 bytes, so we need to allocate more space for the data
            tag_buffer.resize(total_tag_size, 0);

            // Read the remaining data (we already have the first 11 bytes)
            if let Err(e) = reader.read_exact(&mut tag_buffer[11..]) {
                if e.kind() == io::ErrorKind::UnexpectedEof {
                    break;
                }
                return Err(e);
            }

            // Use FlvTag::demux to parse the entire tag
            let tag = FlvTagOwned::demux(&mut Cursor::new(tag_buffer.freeze()))?;
            // tags.push(tag);
            tags_count += 1;
            // Reset buffer for next tag
            tag_buffer = BytesMut::with_capacity(BUFFER_SIZE);
        }

        Ok(tags_count)
    }
}

mod tests {
    use super::*;
    use std::{path::Path, time::Instant};

    #[tokio::test]
    async fn test_read_file() -> Result<(), Box<dyn std::error::Error>> {
        let path = Path::new("D:\\Downloads\\07_47_26-今天能超过10个人吗？.flv");

        // Skip the test if the file doesn't exist
        if !path.exists() {
            println!("Test file not found, skipping test");
            return Ok(());
        }

        // Get file size before parsing
        let file_size = std::fs::metadata(path)?.len();
        let file_size_mb = file_size as f64 / (1024.0 * 1024.0);

        let start = Instant::now(); // Start timer
        let tags_count = FlvParser::parse_file(path)?;
        let duration = start.elapsed(); // Stop timer

        // Calculate read speed
        let seconds = duration.as_secs() as f64 + duration.subsec_nanos() as f64 * 1e-9;
        let speed_mbps = file_size_mb / seconds;

        println!("Parsed FLV file in {:?}", duration);
        println!("File size: {:.2} MB", file_size_mb);
        println!("Read speed: {:.2} MB/s", speed_mbps);

        println!("Successfully parsed FLV file with {} tags", tags_count);

        Ok(())
    }
}
