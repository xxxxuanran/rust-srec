use std::fmt;

use byteorder::{BigEndian, ReadBytesExt};
use bytes::{Buf, Bytes};
use bytes_util::BytesCursorExt;

use super::audio::AudioData;
use super::script::ScriptData;
use super::video::VideoTagHeader;

/// An FLV Tag
///
/// Tags have different types and thus different data structures. To accommodate
/// this the [`FlvTagData`] enum is used.
///
/// Defined by:
/// - video_file_format_spec_v10.pdf (Chapter 1 - The FLV File Format - FLV
///   tags)
/// - video_file_format_spec_v10_1.pdf (Annex E.4.1 - FLV Tag)
///
/// The v10.1 spec adds some additional fields to the tag to accomodate
/// encryption. We dont support this because it is not needed for our use case.
/// (and I suspect it is not used anywhere anymore.)
///
/// However if the Tag is encrypted the tag_type will be a larger number (one we
/// dont support), and therefore the [`FlvTagData::Unknown`] variant will be
/// used.
#[derive(Debug, Clone, PartialEq)]
pub struct FlvTag {
    /// A timestamp in milliseconds
    pub timestamp_ms: u32,
    /// A stream id
    pub stream_id: u32,
    pub data: FlvTagData,
}

impl FlvTag {
    /// Demux a FLV tag from the given reader.
    ///
    /// The reader will be advanced to the end of the tag.
    ///
    /// The reader needs to be a [`std::io::Cursor`] with a [`Bytes`] buffer because we
    /// take advantage of zero-copy reading.
    pub fn demux(reader: &mut std::io::Cursor<Bytes>) -> std::io::Result<Self> {
        // let position = reader.position() as usize;
        let tag_type = FlvTagType::from(reader.read_u8()?);

        let data_size = reader.read_u24::<BigEndian>()?;

        // check if we have the correct amount of bytes to read
        if reader.remaining() < data_size as usize {
            // set the position back to the start of the tag
            // reader.set_position(position as u64);
            return Err(std::io::Error::new(
                std::io::ErrorKind::UnexpectedEof,
                format!(
                    "Not enough bytes to read for tag type {}. Expected {} bytes, got {} bytes",
                    tag_type,
                    data_size,
                    reader.remaining()
                ),
            ));
        }

        // The timestamp bit is weird. Its 24bits but then there is an extended 8 bit
        // number to create a 32bit number.
        let timestamp_ms = reader.read_u24::<BigEndian>()? | ((reader.read_u8()? as u32) << 24);

        // The stream id according to the spec is ALWAYS 0. (likely not true)
        let stream_id = reader.read_u24::<BigEndian>()?;

        // We then extract the data from the reader. (advancing the cursor to the end of
        // the tag)
        let data = reader.extract_bytes(data_size as usize)?;

        // Finally we demux the data.
        let data = FlvTagData::demux(tag_type, &mut std::io::Cursor::new(data))?;

        Ok(FlvTag {
            timestamp_ms,
            stream_id,
            data,
        })
    }
}

impl fmt::Display for FlvTag {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(
            f,
            "FlvTag [Time: {}ms, Stream: {}] {}",
            self.timestamp_ms, self.stream_id, self.data
        )
    }
}

/// FLV Tag Type
///
/// This is the type of the tag.
///
/// Defined by:
/// - video_file_format_spec_v10.pdf (Chapter 1 - The FLV File Format - FLV tags)
/// - video_file_format_spec_v10_1.pdf (Annex E.4.1 - FLV Tag)
///
/// The 3 types that are supported are:
/// - Audio(8)
/// - Video(9)
/// - ScriptData(18)
///
#[repr(u8)]
#[derive(Debug, Clone, PartialEq)]
pub enum FlvTagType {
    Audio = 8,
    Video = 9,
    ScriptData = 18,
    Unknown(u8),
}

impl From<u8> for FlvTagType {
    fn from(value: u8) -> Self {
        match value {
            8 => FlvTagType::Audio,
            9 => FlvTagType::Video,
            18 => FlvTagType::ScriptData,
            _ => FlvTagType::Unknown(value),
        }
    }
}

impl fmt::Display for FlvTagType {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            FlvTagType::Audio => write!(f, "Audio"),
            FlvTagType::Video => write!(f, "Video"),
            FlvTagType::ScriptData => write!(f, "Script"),
            FlvTagType::Unknown(value) => write!(f, "Unknown({})", value),
        }
    }
}

/// FLV Tag Data
///
/// This is a container for the actual media data.
/// This enum contains the data for the different types of tags.
///
/// Defined by:
/// - video_file_format_spec_v10.pdf (Chapter 1 - The FLV File Format - FLV tags)
/// - video_file_format_spec_v10_1.pdf (Annex E.4.1 - FLV Tag)
#[derive(Debug, Clone, PartialEq)]
pub enum FlvTagData {
    /// AudioData when the FlvTagType is Audio(8)
    /// Defined by:
    /// - video_file_format_spec_v10.pdf (Chapter 1 - The FLV File Format - Audio tags)
    /// - video_file_format_spec_v10_1.pdf (Annex E.4.2.1 - AUDIODATA)
    Audio(AudioData),
    /// VideoData when the FlvTagType is Video(9)
    /// Defined by:
    /// - video_file_format_spec_v10.pdf (Chapter 1 - The FLV File Format - Video tags)
    /// - video_file_format_spec_v10_1.pdf (Annex E.4.3.1 - VIDEODATA)
    Video(VideoTagHeader),
    /// ScriptData when the FlvTagType is ScriptData(18)
    /// Defined by:
    /// - video_file_format_spec_v10.pdf (Chapter 1 - The FLV File Format - Data tags)
    /// - video_file_format_spec_v10_1.pdf (Annex E.4.4.1 - SCRIPTDATA)
    ScriptData(ScriptData),
    /// Any tag type that we dont know how to parse, with the corresponding data
    /// being the raw bytes of the tag
    Unknown { tag_type: FlvTagType, data: Bytes },
}

impl FlvTagData {
    /// Demux a FLV tag data from the given reader.
    ///
    /// The reader will be enirely consumed.
    ///
    /// The reader needs to be a [`std::io::Cursor`] with a [`Bytes`] buffer because we
    /// take advantage of zero-copy reading.
    pub fn demux(
        tag_type: FlvTagType,
        reader: &mut std::io::Cursor<Bytes>,
    ) -> std::io::Result<Self> {
        match tag_type {
            FlvTagType::Audio => Ok(FlvTagData::Audio(AudioData::demux(reader, None)?)),
            FlvTagType::Video => Ok(FlvTagData::Video(VideoTagHeader::demux(reader)?)),
            FlvTagType::ScriptData => Ok(FlvTagData::ScriptData(ScriptData::demux(reader)?)),
            _ => Ok(FlvTagData::Unknown {
                tag_type,
                data: reader.extract_remaining(),
            }),
        }
    }
}

impl fmt::Display for FlvTagData {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            FlvTagData::Audio(audio) => write!(f, "Audio: {}", audio),
            FlvTagData::Video(video) => write!(f, "Video: {}", video),
            FlvTagData::ScriptData(script) => write!(f, "Script: {}", script),
            FlvTagData::Unknown { tag_type, data } => {
                write!(f, "Unknown (type: {:?}, {} bytes)", tag_type, data.len())
            }
        }
    }
}
