/// ISO/IEC 23008-2 - Table E.2
#[derive(Debug, Clone, PartialEq, Copy)]
#[repr(u8)]
pub enum VideoFormat {
    /// Component
    Component = 0,
    /// PAL
    PAL = 1,
    /// NTSC
    NTSC = 2,
    /// SECAM
    SECAM = 3,
    /// MAC
    MAC = 4,
    /// Unspecified video format
    Unspecified = 5,
}

impl From<u8> for VideoFormat {
    fn from(value: u8) -> Self {
        match value {
            0 => VideoFormat::Component,
            1 => VideoFormat::PAL,
            2 => VideoFormat::NTSC,
            3 => VideoFormat::SECAM,
            4 => VideoFormat::MAC,
            5 => VideoFormat::Unspecified,
            _ => VideoFormat::Unspecified,
        }
    }
}
