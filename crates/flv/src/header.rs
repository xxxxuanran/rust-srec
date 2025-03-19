use std::fmt::Display;
use std::io;

use byteorder::{BigEndian, ReadBytesExt};
use bytes::Bytes;

const FLV_HEADER_SIZE: usize = 9;

// Struct representing the FLV header, 9 bytes in total
#[derive(Debug, Clone, PartialEq)]
pub struct FlvHeader {
    pub signature: u32, // The signature of the FLV file, 3 bytes, always 'FLV'
    // The version of the FLV file format, 1 byte, usually 0x01
    pub version: u8,
    // Whether the FLV file contains audio data, 1 byte
    pub has_audio: bool,
    // Whether the FLV file contains video data, 1 byte
    pub has_video: bool,
    // Total size of the header, 4 bytes, always 0x09
    pub data_offset: Bytes,
}

impl Display for FlvHeader {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        // Convert signature to a string (FLV)
        let signature_string = format!(
            "{}{}{}",
            ((self.signature >> 16) & 0xFF) as u8 as char,
            ((self.signature >> 8) & 0xFF) as u8 as char,
            (self.signature & 0xFF) as u8 as char
        );

        write!(
            f,
            "FLV Header: \n\
            Signature: {}\n\
            Version: {}\n\
            Has Audio: {}\n\
            Has Video: {}\n\
            Data Offset: {}",
            signature_string,
            self.version,
            self.has_audio,
            self.has_video,
            self.data_offset.len()
        )
    }
}

impl FlvHeader {
    /// Parses the FLV header from a byte stream.
    /// Returns a `FlvHeader` struct if successful, or an error if the header is invalid.
    /// The function reads the first 9 bytes of the stream and checks for the FLV signature.
    /// If the signature is not 'FLV', it returns an error.
    /// The function also checks if the data offset is valid and returns an error if it is not.
    ///
    /// This function can return an `io::Error` if buffer is not enough or if the header is invalid.
    /// Arguments:
    /// - `reader`: A mutable reference to a `Cursor<Bytes>` that contains the byte stream.
    /// The reader will be advanced to the end of the header.
    ///
    /// The reader needs to be a [`std::io::Cursor`] with a [`Bytes`] buffer because we
    /// take advantage of zero-copy reading.
    pub fn parse(reader: &mut io::Cursor<Bytes>) -> io::Result<Self> {
        let start = reader.position() as usize;

        // Signature is a 3-byte string 'FLV'
        let signature = reader.read_u24::<BigEndian>()?;

        // compare if signature is 'FLV'
        if signature != 0x464C56 {
            // move the cursor back to the start position
            reader.set_position(start as u64);
            return Err(io::Error::new(
                io::ErrorKind::InvalidData,
                "Invalid FLV signature",
            ));
        }

        // Version is a 1-byte value
        let version = reader.read_u8()?;
        // Flags is a 1-byte value
        let flags = reader.read_u8()?;
        // Has audio and video flags
        let has_audio = flags & 0b00000100 != 0;
        let has_video = flags & 0b00000001 != 0;

        // Data offset is a 4-byte value
        let data_offset = reader.read_u32::<BigEndian>()? as usize;

        let end = reader.position() as usize;
        // Check if the data offset is valid
        let size = end - start;

        if size < FLV_HEADER_SIZE || data_offset != FLV_HEADER_SIZE {
            return Err(io::Error::new(
                io::ErrorKind::InvalidData,
                format!("Invalid FLV header size: {}", size),
            ));
        }

        // O[1] slice operation to extract data_offset
        let offset = reader.get_ref().slice(data_offset..end);

        Ok(FlvHeader {
            signature,
            version,
            has_audio,
            has_video,
            data_offset: offset,
        })
    }
}

#[cfg(test)]
mod tests {
    use crate::header::FlvHeader;
    use byteorder::{BigEndian, WriteBytesExt};
    use bytes::{Bytes, BytesMut};
    use std::io::Cursor;

    #[test]
    fn test_valid_flv_header() {
        // Create a buffer with a valid FLV header
        let mut buffer = BytesMut::new();

        // Write "FLV" signature (3 bytes)
        buffer.extend_from_slice(b"FLV");

        // Write version (1 byte)
        buffer.extend_from_slice(&[0x01]);

        // Write flags (1 byte - both audio and video)
        buffer.extend_from_slice(&[0x03]);

        // Write data offset (4 bytes - standard 9)
        let mut offset_bytes = vec![];
        offset_bytes.write_u32::<BigEndian>(9).unwrap();
        buffer.extend_from_slice(&offset_bytes);

        // Create a cursor for reading
        let bytes = buffer.freeze();
        let mut reader = Cursor::new(bytes);

        // Parse the header
        let header = FlvHeader::parse(&mut reader).unwrap();

        // Verify the parsed values
        assert_eq!(header.signature, 0x464C56); // "FLV" in hex
        assert_eq!(header.version, 0x01);
        assert!(header.has_audio);
        assert!(header.has_video);
        assert_eq!(reader.position(), 9); // Reader should be at position 9
    }

    #[test]
    fn test_invalid_flv_signature() {
        // Create a buffer with an invalid signature
        let mut buffer = BytesMut::new();

        // Write invalid signature "ABC" instead of "FLV"
        buffer.extend_from_slice(b"ABC");

        // Add remaining header bytes
        buffer.extend_from_slice(&[0x01, 0x03]);
        let mut offset_bytes = vec![];
        offset_bytes.write_u32::<BigEndian>(9).unwrap();
        buffer.extend_from_slice(&offset_bytes);

        // Create a cursor for reading
        let bytes = buffer.freeze();
        let mut reader = Cursor::new(bytes);

        // Parse should fail with invalid signature
        let result = FlvHeader::parse(&mut reader);
        assert!(result.is_err());

        // Verify reader position is reset to start
        assert_eq!(reader.position(), 0);
    }

    #[test]
    fn test_header_with_audio_only() {
        // Create a buffer with audio-only flag
        let mut buffer = BytesMut::new();
        buffer.extend_from_slice(b"FLV");
        buffer.extend_from_slice(&[0x01, 0x01]); // Version 1, Audio only flag

        let mut offset_bytes = vec![];
        offset_bytes.write_u32::<BigEndian>(9).unwrap();
        buffer.extend_from_slice(&offset_bytes);

        let bytes = buffer.freeze();
        let mut reader = Cursor::new(bytes);

        let header = FlvHeader::parse(&mut reader).unwrap();

        assert!(header.has_audio);
        assert!(!header.has_video);
    }

    #[test]
    fn test_header_with_video_only() {
        // Create a buffer with video-only flag
        let mut buffer = BytesMut::new();
        buffer.extend_from_slice(b"FLV");
        buffer.extend_from_slice(&[0x01, 0x02]); // Version 1, Video only flag

        let mut offset_bytes = vec![];
        offset_bytes.write_u32::<BigEndian>(9).unwrap();
        buffer.extend_from_slice(&offset_bytes);

        let bytes = buffer.freeze();
        let mut reader = Cursor::new(bytes);

        let header = FlvHeader::parse(&mut reader).unwrap();

        assert!(!header.has_audio);
        assert!(header.has_video);
    }

    #[test]
    fn test_invalid_data_offset() {
        // Create a buffer with an invalid data offset (smaller than header size)
        let mut buffer = BytesMut::new();
        buffer.extend_from_slice(b"FLV");
        buffer.extend_from_slice(&[0x01, 0x03]);

        // Invalid offset (4 instead of 9)
        let mut offset_bytes = vec![];
        offset_bytes.write_u32::<BigEndian>(4).unwrap();
        buffer.extend_from_slice(&offset_bytes);

        let bytes = buffer.freeze();
        let mut reader = Cursor::new(bytes);

        // Should still parse but with a warning (current implementation)
        let parse_result = FlvHeader::parse(&mut reader);
        assert!(parse_result.is_err());
        let error = parse_result.unwrap_err();
        assert_eq!(error.kind(), std::io::ErrorKind::InvalidData);
    }
}
