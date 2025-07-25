/// The number of temporal layers in the stream.
///
/// `0` and `1` are special values.
///
/// Any other value represents the actual number of temporal layers.
#[derive(Debug, Clone, PartialEq, Copy, PartialOrd, Ord, Eq)]
#[repr(u8)]
pub enum NumTemporalLayers {
    /// The stream might be temporally scalable.
    Unknown = 0,
    /// The stream is not temporally scalable.
    NotScalable = 1,
}

impl From<u8> for NumTemporalLayers {
    fn from(value: u8) -> Self {
        match value {
            0 => NumTemporalLayers::Unknown,
            1 => NumTemporalLayers::NotScalable,
            _ => panic!("invalid num_temporal_layers: {value}"),
        }
    }
}
