/// Indicates the type of parallelism that is used to meet the restrictions imposed
/// by [`min_spatial_segmentation_idc`](crate::HEVCDecoderConfigurationRecord::min_spatial_segmentation_idc) when the value of
/// [`min_spatial_segmentation_idc`](crate::HEVCDecoderConfigurationRecord::min_spatial_segmentation_idc) is greater than 0.
///
/// ISO/IEC 14496-15 - 8.3.2.1.3
#[derive(Debug, Clone, PartialEq, Copy)]
#[repr(u8)]
pub enum ParallelismType {
    /// The stream supports mixed types of parallel decoding or the parallelism type is unknown.
    MixedOrUnknown = 0,
    /// The stream supports slice based parallel decoding.
    Slice = 1,
    /// The stream supports tile based parallel decoding.
    Tile = 2,
    /// The stream supports entropy coding sync based parallel decoding.
    EntropyCodingSync = 3,
}

impl From<u8> for ParallelismType {
    fn from(value: u8) -> Self {
        match value {
            0 => ParallelismType::MixedOrUnknown,
            1 => ParallelismType::Slice,
            2 => ParallelismType::Tile,
            3 => ParallelismType::EntropyCodingSync,
            _ => panic!("invalid parallelism_type: {value}"),
        }
    }
}
