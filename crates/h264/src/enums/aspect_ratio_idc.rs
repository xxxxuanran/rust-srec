use std::fmt;

/// The `AspectRatioIdc` is a nutype enum for `aspect_ratio_idc` as defined in
/// ISO/IEC-14496-10-2022 - E.2.1 Table E-1.
///
/// Values 17..=254** are reserved (should be ignored if encountered)
/// **Value 255 (`ExtendedSar`)** indicates that the aspect ratio is specified by
///   additional fields (`sar_width` and `sar_height`) in the bitstream.
///
/// ## Examples of aspect_ratio_idc values:
/// - `1` => 1:1 ("square")
/// - `4` => 16:11
/// - `14` => 4:3
/// - `15` => 3:2
/// - `16` => 2:1

#[derive(Clone, Copy, PartialEq)]
#[repr(u8)]
pub enum AspectRatioIdc {
    /// 0: Unspecified (not used in decoding)
    Unspecified = 0,

    /// 1: 1:1 (square)
    /// ## Examples
    /// - 7680 x 4320 16:9 w/o horizontal overscan
    /// - 3840 x 2160 16:9 w/o horizontal overscan
    /// - 1280 x 720 16:9 w/o horizontal overscan
    /// - 1920 x 1080 16:9 w/o horizontal overscan (cropped from 1920x1088)
    /// - 640 x 480 4:3 w/o horizontal overscan
    Square = 1,

    /// 2: 12:11
    /// ## Examples
    /// - 720 x 576 4:3 with horizontal overscan
    /// - 352 x 288 4:3 w/o horizontal overscan
    Aspect12_11 = 2,

    /// 3: 10:11
    /// ## Examples
    /// - 720 x 480 4:3 with horizontal overscan
    /// - 352 x 240 4:3 w/o horizontal overscan
    Aspect10_11 = 3,

    /// 4: 16:11
    /// ## Examples
    /// - 720 x 576 16:9 with horizontal overscan
    /// - 528 x 576 4:3 w/o horizontal overscan
    Aspect16_11 = 4,

    /// 5: 40:33
    /// ## Examples
    /// - 720 x 480 16:9 with horizontal overscan
    /// - 528 x 480 4:3 w/o horizontal overscan
    Aspect40_33 = 5,

    /// 6: 24:11
    /// ## Examples
    /// - 352 x 576 4:3 w/o horizontal overscan
    /// - 480 x 576 16:9 with horizontal overscan
    Aspect24_11 = 6,

    /// 7: 20:11
    /// ## Examples
    /// - 352 x 480 4:3 w/o horizontal overscan
    /// - 480 x 480 16:9 with horizontal overscan
    Aspect20_11 = 7,

    /// 8: 32:11
    /// ## Example
    /// - 352 x 576 16:9 w/o horizontal overscan
    Aspect32_11 = 8,

    /// 9: 80:33
    /// ## Example
    /// - 352 x 480 16:9 w/o horizontal overscan
    Aspect80_33 = 9,

    /// 10: 18:11
    /// ## Example
    /// - 480 x 576 16:9 with horizontal overscan
    Aspect18_11 = 10,

    /// 11: 15:11
    /// ## Example
    /// - 480 x 480 4:3 with horizontal overscan
    Aspect15_11 = 11,

    /// 12: 64:33
    /// ## Example
    /// - 528 x 576 16:9 w/o horizontal overscan
    Aspect64_33 = 12,

    /// 13: 160:99
    /// ## Example
    /// - 528 x 480 16:9 w/o horizontal overscan
    Aspect160_99 = 13,

    /// 14: 4:3
    /// ## Example
    /// - 1440 x 1080 16:9 w/o horizontal overscan
    Aspect4_3 = 14,

    /// 15: 3:2
    /// ## Example
    /// - 1280 x 1080 16:9 w/o horizontal overscan
    Aspect3_2 = 15,

    /// 16: 2:1
    /// ## Example
    /// - 960 x 1080 16:9 w/o horizontal overscan
    Aspect2_1 = 16,

    /// 17..=254: Reserved (should be ignored)
    Reserved = 17,

    /// 255: Extended SAR (use `sar_width` & `sar_height` from bitstream)
    ExtendedSar = 255,
}

// Implement Debug manually to ensure it includes the enum name in output
impl fmt::Debug for AspectRatioIdc {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "AspectRatioIdc::")?;
        match self {
            Self::Unspecified => write!(f, "Unspecified"),
            Self::Square => write!(f, "Square"),
            Self::Aspect12_11 => write!(f, "Aspect12_11"),
            Self::Aspect10_11 => write!(f, "Aspect10_11"),
            Self::Aspect16_11 => write!(f, "Aspect16_11"),
            Self::Aspect40_33 => write!(f, "Aspect40_33"),
            Self::Aspect24_11 => write!(f, "Aspect24_11"),
            Self::Aspect20_11 => write!(f, "Aspect20_11"),
            Self::Aspect32_11 => write!(f, "Aspect32_11"),
            Self::Aspect80_33 => write!(f, "Aspect80_33"),
            Self::Aspect18_11 => write!(f, "Aspect18_11"),
            Self::Aspect15_11 => write!(f, "Aspect15_11"),
            Self::Aspect64_33 => write!(f, "Aspect64_33"),
            Self::Aspect160_99 => write!(f, "Aspect160_99"),
            Self::Aspect4_3 => write!(f, "Aspect4_3"),
            Self::Aspect3_2 => write!(f, "Aspect3_2"),
            Self::Aspect2_1 => write!(f, "Aspect2_1"),
            Self::Reserved => write!(f, "Reserved"),
            Self::ExtendedSar => write!(f, "ExtendedSar"),
        }
    }
}

impl TryFrom<u8> for AspectRatioIdc {
    type Error = &'static str;

    /// Converts a `u8` value to an `AspectRatioIdc`.
    /// Returns an error if the value is reserved (17..=254).
    fn try_from(value: u8) -> Result<Self, Self::Error> {
        match value {
            0 => Ok(AspectRatioIdc::Unspecified),
            1 => Ok(AspectRatioIdc::Square),
            2 => Ok(AspectRatioIdc::Aspect12_11),
            3 => Ok(AspectRatioIdc::Aspect10_11),
            4 => Ok(AspectRatioIdc::Aspect16_11),
            5 => Ok(AspectRatioIdc::Aspect40_33),
            6 => Ok(AspectRatioIdc::Aspect24_11),
            7 => Ok(AspectRatioIdc::Aspect20_11),
            8 => Ok(AspectRatioIdc::Aspect32_11),
            9 => Ok(AspectRatioIdc::Aspect80_33),
            10 => Ok(AspectRatioIdc::Aspect18_11),
            11 => Ok(AspectRatioIdc::Aspect15_11),
            12 => Ok(AspectRatioIdc::Aspect64_33),
            13 => Ok(AspectRatioIdc::Aspect160_99),
            14 => Ok(AspectRatioIdc::Aspect4_3),
            15 => Ok(AspectRatioIdc::Aspect3_2),
            16 => Ok(AspectRatioIdc::Aspect2_1),
            17..=254 => Err("Reserved value"),
            255 => Ok(AspectRatioIdc::ExtendedSar),
        }
    }
}

impl From<AspectRatioIdc> for u8 {
    fn from(value: AspectRatioIdc) -> Self {
        value as u8
    }
}
