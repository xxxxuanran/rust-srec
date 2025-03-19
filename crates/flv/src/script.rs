//! # FLV Script Module
//!
//! Implementation of FLV script tag data parsing for metadata and other scripting information.
//!
//! This module handles parsing of script data from FLV (Flash Video) files, primarily used
//! for metadata such as video dimensions, duration, framerate, and other properties. Script
//! tags contain ActionScript objects serialized in AMF0 format.
//!
//! ## Features
//!
//! - Parses AMF0-encoded script data from FLV tags
//! - Extracts metadata fields like duration, width, height, framerate, etc.
//! - Supports common script tag names like "onMetaData"
//! - Handles complex nested data structures through the AMF0 format
//!
//! ## Common Script Tags
//!
//! - `onMetaData`: Contains metadata about the media file (dimensions, duration, etc.)
//!
//! ## Specifications
//!
//! - [Flash Video File Format Specification v10](https://www.adobe.com/content/dam/acom/en/devnet/flv/video_file_format_spec_v10.pdf)
//! - [Action Message Format -- AMF 0](https://www.adobe.com/content/dam/acom/en/devnet/pdf/amf0-file-format-specification.pdf)
//!
//! ## Usage
//!
//! ```
//! use flv::script::ScriptData;
//! use bytes::Bytes;
//! use std::io::Cursor;
//!
//! // Parse script data from an FLV tag body
//! let data = vec![/* FLV script tag data */];
//! let mut cursor = Cursor::new(Bytes::from(data));
//! let script = ScriptData::demux(&mut cursor).unwrap();
//!
//! // Check script name and access data
//! if script.name == "onMetaData" {
//!     println!("Found metadata script tag with {} values", script.data.len());
//! }
//! ```
//! ## Credits
//!
//! Based on specifications from Adobe and the E-RTMP project.
//!
//! Based on the work of [ScuffleCloud project](https://github.com/ScuffleCloud/scuffle/blob/main/crates/flv/src/script.rs)
//!
//! ## License
//!
//! MIT License
//!
//! ## Authors
//!
//! - ScuffleCloud project contributors
//! - hua0512

use std::io;

use amf0::{Amf0Decoder, Amf0Marker, Amf0Value};
use bytes::Bytes;
use bytes_util::BytesCursorExt;

#[derive(Debug, Clone, PartialEq)]
pub struct ScriptData {
    /// The name of the script data
    pub name: String,
    /// The data of the script data
    pub data: Vec<Amf0Value<'static>>,
}

impl ScriptData {
    pub fn demux(reader: &mut io::Cursor<Bytes>) -> io::Result<Self> {
        let buf = reader.extract_remaining();
        let mut amf0_reader = Amf0Decoder::new(&buf);

        let name = match amf0_reader.decode_with_type(Amf0Marker::String) {
            Ok(Amf0Value::String(name)) => name,
            _ => {
                return Err(io::Error::new(
                    io::ErrorKind::InvalidData,
                    "Invalid script data name",
                ));
            }
        };

        let data = amf0_reader
            .decode_all()
            .map_err(|_| io::Error::new(io::ErrorKind::InvalidData, "Invalid script data"))?;

        Ok(Self {
            name: name.into_owned(),
            data: data.into_iter().map(|v| v.to_owned()).collect(),
        })
    }
}

#[cfg(test)]
#[cfg_attr(all(test, coverage_nightly), coverage(off))]
mod tests {
    use super::*;

    #[test]
    fn test_script_data() {
        let mut reader = io::Cursor::new(Bytes::from_static(&[
            0x02, // String marker
            0x00, 0x0A, // Length (10 bytes)
            b'o', b'n', b'M', b'e', b't', b'a', b'D', b'a', b't', b'a', // "onMetaData"
            0x05, // null marker
            0x05, // null marker
        ]));
        let script_data = ScriptData::demux(&mut reader).unwrap();
        assert_eq!(script_data.name, "onMetaData");
        assert_eq!(script_data.data.len(), 2);
        assert_eq!(script_data.data[0], Amf0Value::Null);
        assert_eq!(script_data.data[1], Amf0Value::Null);
    }
}
