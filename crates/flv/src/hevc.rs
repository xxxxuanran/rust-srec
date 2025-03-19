use std::io;

use byteorder::{BigEndian, ReadBytesExt};
use bytes::Bytes;
use bytes_util::BytesCursorExt;
use h265::HEVCDecoderConfigurationRecord;

#[repr(u8)]
#[derive(Debug, Clone, PartialEq)]
pub enum HevcPacketType {
    /// HEVC Sequence Header
    SeqHdr = 0,
    /// HEVC NALU
    Nalu = 1,
    /// HEVC End of Sequence
    EndOfSequence = 2,

    Unknown(u8),
}

impl TryFrom<u8> for HevcPacketType {
    type Error = io::Error;

    fn try_from(value: u8) -> Result<Self, Self::Error> {
        match value {
            0 => Ok(Self::SeqHdr),
            1 => Ok(Self::Nalu),
            2 => Ok(Self::EndOfSequence),
            _ => Ok(Self::Unknown(value)),
        }
    }
}

/// HEVC Packet
#[derive(Debug, Clone, PartialEq)]
pub enum HevcPacket {
    /// HEVC Sequence Start
    SequenceStart(HEVCDecoderConfigurationRecord),
    /// HEVC NALU
    Nalu {
        composition_time: Option<i32>,
        data: Bytes,
    },

    /// HEVC End of Sequence
    /// End of Sequence
    EndOfSequence,
    /// HEVC Unknown (we don't know how to parse it)
    /// Unknown
    Unknown {
        hevc_packet_type: HevcPacketType,
        composition_time: Option<i32>,
        data: Bytes,
    },
}

impl HevcPacket {
    /// Demux HEVC packet
    pub fn demux(reader: &mut io::Cursor<Bytes>) -> io::Result<Self> {
        let hevc_packet_type = HevcPacketType::try_from(reader.read_u8()?)?;
        let composition_time = if hevc_packet_type == HevcPacketType::Nalu {
            Some(reader.read_i24::<BigEndian>()?)
        } else {
            None
        };

        match hevc_packet_type {
            HevcPacketType::SeqHdr => Ok(Self::SequenceStart(
                HEVCDecoderConfigurationRecord::demux(reader)?,
            )),
            HevcPacketType::Nalu => Ok(Self::Nalu {
                composition_time,
                data: reader.extract_remaining(),
            }),
            HevcPacketType::EndOfSequence => Ok(Self::EndOfSequence),
            _ => Ok(Self::Unknown {
                hevc_packet_type,
                composition_time,
                data: reader.extract_remaining(),
            }),
        }
    }
}
