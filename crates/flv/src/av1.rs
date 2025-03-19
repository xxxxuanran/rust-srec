use av1::AV1CodecConfigurationRecord;
use bytes::Bytes;

/// AV1 Packet
/// This is a container for av1 data.
/// This enum contains the data for the different types of av1 packets.
#[derive(Debug, Clone, PartialEq)]
pub enum Av1Packet {
    /// AV1 Sequence Start
    SequenceStart(AV1CodecConfigurationRecord),
    /// AV1 Raw Data
    Raw(Bytes),
}

// Display traits for the packet types
impl std::fmt::Display for Av1Packet {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Av1Packet::SequenceStart(config) => write!(
                f,
                "SequenceStart [Profile: {}, Level: {}]",
                config.seq_profile, config.seq_level_idx_0
            ),
            Av1Packet::Raw(data) => write!(f, "Data ({} bytes)", data.len()),
        }
    }
}
