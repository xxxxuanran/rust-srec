/// Represents all possible values of the `constant_frame_rate` field in the
/// [`HEVCDecoderConfigurationRecord`](crate::config::HEVCDecoderConfigurationRecord).
///
/// ISO/IEC 14496-15 - 8.3.2.1.3
#[derive(Debug, Clone, PartialEq, Copy, PartialOrd, Ord, Eq)]
#[repr(u8)]
pub enum ConstantFrameRate {
    /// Indicates that the stream may or may not be of constant frame rate.
    Unknown = 0,
    /// Indicates that the stream to which this configuration record
    /// applies is of constant frame rate.
    Constant = 1,
    /// Indicates that the representation of each temporal
    /// layer in the stream is of constant frame rate.
    TemporalLayerConstant = 2,
}

impl From<u8> for ConstantFrameRate {
    fn from(value: u8) -> Self {
        match value {
            0 => ConstantFrameRate::Unknown,
            1 => ConstantFrameRate::Constant,
            2 => ConstantFrameRate::TemporalLayerConstant,
            _ => panic!("invalid constant_frame_rate: {value}"),
        }
    }
}
