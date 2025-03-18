// This 

use std::io;

use bytes::Bytes;

#[derive(Debug, Clone, PartialEq)]
pub enum AacPacketType{
    /// AAC Sequence Header
    SequenceHeader = 0x00,
    /// AAC Raw
    Raw = 0x01,
}

impl TryFrom<u8> for AacPacketType {
    type Error = io::Error;

    fn try_from(value: u8) -> Result<Self, Self::Error> {
        match value {
            0x00 => Ok(AacPacketType::SequenceHeader),
            0x01 => Ok(AacPacketType::Raw),
            _ => Err(io::Error::new(
                io::ErrorKind::InvalidData,
                format!("Invalid AAC packet type: {}", value),
            )),
        }
    }
}



impl AacPacketType {
    /// Create a new AacPacketType from the given value
    pub fn new(value: u8) -> Option<Self> {
        match value {
            0x00 => Some(AacPacketType::SequenceHeader),
            0x01 => Some(AacPacketType::Raw),
            _ => None
        }
    }

}



/// AAC Packet
/// This is a container for aac data.
/// This enum contains the data for the different types of aac packets.
/// Defined in the FLV specification. Chapter 1 - AACAUDIODATA
#[derive(Debug, Clone, PartialEq)]
pub enum AacPacket {
    /// AAC Sequence Header
    SequenceHeader(Bytes),
    /// AAC Raw
    Raw(Bytes),
    /// Data we don't know how to parse
    Unknown { aac_packet_type: AacPacketType, data: Bytes },
}

impl AacPacket {
    /// Create a new AAC packet from the given data and packet type
    pub fn new(aac_packet_type: AacPacketType, data: Bytes) -> Self {
        match aac_packet_type {
            AacPacketType::Raw => AacPacket::Raw(data),
            AacPacketType::SequenceHeader => AacPacket::SequenceHeader(data),
            // _ => AacPacket::Unknown { aac_packet_type, data },
        }
    }
}

#[cfg(test)]
#[cfg_attr(all(test, coverage_nightly), coverage(off))]
mod tests {
    use super::*;

    #[test]
    fn test_new() {
        let cases = [
            (
                AacPacketType::Raw,
                Bytes::from(vec![0, 1, 2, 3]),
                AacPacket::Raw(Bytes::from(vec![0, 1, 2, 3])),
            ),
            (
                AacPacketType::SequenceHeader,
                Bytes::from(vec![0, 1, 2, 3]),
                AacPacket::SequenceHeader(Bytes::from(vec![0, 1, 2, 3])),
            ),
            (
                AacPacketType::SequenceHeader,
                Bytes::from(vec![0, 1, 2, 3]),
                AacPacket::SequenceHeader(Bytes::from(vec![0, 1, 2, 3])),
            ),
            (
                AacPacketType::Raw,
                Bytes::from(vec![0, 1, 2, 3]),
                AacPacket::Raw(Bytes::from(vec![0, 1, 2, 3])),
            ),
            (
                AacPacketType::new(0x2).unwrap_or(AacPacketType::Raw),
                Bytes::from(vec![0, 1, 2, 3]),
                AacPacket::Unknown {
                    aac_packet_type: AacPacketType::new(0x2).unwrap_or(AacPacketType::Raw),
                    data: Bytes::from(vec![0, 1, 2, 3]),
                },
            ),
            (
                AacPacketType::new(0x3).unwrap_or(AacPacketType::Raw),
                Bytes::from(vec![0, 1, 2, 3]),
                AacPacket::Unknown {
                    aac_packet_type: AacPacketType::new(0x3).unwrap_or(AacPacketType::Raw),
                    data: Bytes::from(vec![0, 1, 2, 3]),
                },
            ),
        ];

        for (packet_type, data, expected) in cases {
            let packet = AacPacket::new(packet_type, data.clone());
            assert_eq!(packet, expected);
        }
    }

    #[test]
    fn test_aac_packet_type() {
        assert_eq!(
            format!("{:?}", AacPacketType::SequenceHeader),
            "AacPacketType::SequenceHeader"
        );
        assert_eq!(format!("{:?}", AacPacketType::Raw), "AacPacketType::Raw");
        let packet_type_2 = AacPacketType::new(0x2).unwrap_or(AacPacketType::Raw);
        let packet_type_3 = AacPacketType::new(0x3).unwrap_or(AacPacketType::Raw);
        assert_eq!(format!("{:?}", packet_type_2), "Raw");
        assert_eq!(format!("{:?}", packet_type_3), "Raw");

        assert_eq!(AacPacketType::new(0x01).unwrap(), AacPacketType::Raw);
        assert_eq!(AacPacketType::new(0x00).unwrap(), AacPacketType::SequenceHeader);
    }
}