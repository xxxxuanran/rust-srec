use byteorder::{BigEndian, ReadBytesExt};
use nom::{
    IResult,
    bytes::complete::take,
    combinator::map,
    number::complete::{be_u8, be_u24, be_u32},
    sequence::tuple,
};
use std::io::{self, Read, Seek, SeekFrom};

#[derive(Debug)]
struct FlvHeader {
    signature: [u8; 3], // "FLV"
    version: u8,
    flags: u8,
    data_offset: u32,
}

#[derive(Debug)]
struct FlvTag<'a> {
    tag_type: u8,
    data_size: u32,
    timestamp: u32,
    stream_id: u32,
    payload: &'a [u8], // Zero-copy reference to the payload
}

fn parse_flv_header(input: &[u8]) -> IResult<&[u8], FlvHeader> {
    let (input, signature) = take(3usize)(input)?; // Read "FLV"
    let (input, version) = be_u8(input)?; // Read version
    let (input, flags) = be_u8(input)?; // Read flags
    let (input, data_offset) = be_u32(input)?; // Read data offset

    let signature_array: [u8; 3] = [signature[0], signature[1], signature[2]];

    Ok((
        input,
        FlvHeader {
            signature: signature_array,
            version,
            flags,
            data_offset,
        },
    ))
}

fn parse_flv_tag(input: &[u8]) -> nom::IResult<&[u8], FlvTag> {
    let (input, (tag_type, data_size, timestamp, timestamp_extended, stream_id)) = tuple((
        be_u8,  // Tag type
        be_u24, // Data size
        be_u24, // Timestamp (lower 24 bits)
        be_u8,  // Timestamp extended (upper 8 bits)
        be_u24, // Stream ID (always 0)
    ))(input)?;

    let (input, payload) = take(data_size)(input)?; // Payload data

    Ok((
        input,
        FlvTag {
            tag_type,
            data_size,
            timestamp: (u32::from(timestamp_extended) << 24 | timestamp),
            stream_id,
            payload,
        },
    ))
}

mod test {
    use std::fs::File;
    use std::io::{self, BufReader, Read, SeekFrom};
    use std::path::Path;
    use std::time::Instant;

    use crate::buffer::{parse_flv_header, parse_flv_tag};

    #[test]
    fn test_file() -> io::Result<()> {
        let path = Path::new("D:\\Downloads\\07_47_26-今天能超过10个人吗？.flv");
        let file = File::open(path)?;

        // Get file size before parsing
        let file_size = std::fs::metadata(path)?.len();
        let file_size_mb = file_size as f64 / (1024.0 * 1024.0);

        let start = Instant::now(); // Start timer

        let mut reader = BufReader::new(file);

        // Read the header
        let mut header_buf = [0u8; 9]; // FLV header is 9 bytes
        reader.read_exact(&mut header_buf)?;

        let (_, header) = parse_flv_header(&header_buf).map_err(|e| {
            io::Error::new(
                io::ErrorKind::InvalidData,
                format!("Failed to parse header: {:?}", e),
            )
        })?;
        // println!("{:?}", header);

        // Skip to the first tag
        reader.seek_relative((header.data_offset - 9) as i64)?;
        let mut tags_count = 0;

        // Parse tags incrementally
        let mut tag_buf = vec![0u8; 1024 * 1024]; // 1 MB buffer
        loop {
            let bytes_read = reader.read(&mut tag_buf)?;
            if bytes_read == 0 {
                break; // End of file
            }

            // Scope the immutable borrow of `tag_buf`
            let (remaining, tag) = {
                let input = &tag_buf[..bytes_read];
                parse_flv_tag(input).map_err(|e| {
                    io::Error::new(
                        io::ErrorKind::InvalidData,
                        format!("Failed to parse tag: {:?}", e),
                    )
                })?
            };

            // println!("{:?}", tag);
            tags_count += 1;

            // Handle remaining data (if any)
            if !remaining.is_empty() {
                // Move remaining data to the beginning of the buffer
                let remaining_len = remaining.len();
                tag_buf.copy_within(bytes_read - remaining_len..bytes_read, 0);
            }
        }

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
