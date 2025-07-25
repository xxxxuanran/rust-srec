/// Interpretation of sample aspect ratio indicator.
///
/// ISO/IEC 23008-2 - Table E.1
#[derive(Debug, Clone, PartialEq)]
#[repr(u8)]
pub enum AspectRatioIdc {
    /// Unspecified
    Unspecified = 0,
    /// 1:1 (square)
    Square = 1,
    /// 12:11
    Aspect12_11 = 2,
    /// 10:11
    Aspect10_11 = 3,
    /// 16:11
    Aspect16_11 = 4,
    /// 40:33
    Aspect40_33 = 5,
    /// 24:11
    Aspect24_11 = 6,
    /// 20:11
    Aspect20_11 = 7,
    /// 32:11
    Aspect32_11 = 8,
    /// 80:33
    Aspect80_33 = 9,
    /// 18:11
    Aspect18_11 = 10,
    /// 15:11
    Aspect15_11 = 11,
    /// 64:33
    Aspect64_33 = 12,
    /// 160:99
    Aspect160_99 = 13,
    /// 4:3
    Aspect4_3 = 14,
    /// 3:2
    Aspect3_2 = 15,
    /// 2:1
    Aspect2_1 = 16,
    /// EXTENDED_SAR
    ExtendedSar = 255,
}

impl From<u8> for AspectRatioIdc {
    fn from(value: u8) -> Self {
        match value {
            0 => AspectRatioIdc::Unspecified,
            1 => AspectRatioIdc::Square,
            2 => AspectRatioIdc::Aspect12_11,
            3 => AspectRatioIdc::Aspect10_11,
            4 => AspectRatioIdc::Aspect16_11,
            5 => AspectRatioIdc::Aspect40_33,
            6 => AspectRatioIdc::Aspect24_11,
            7 => AspectRatioIdc::Aspect20_11,
            8 => AspectRatioIdc::Aspect32_11,
            9 => AspectRatioIdc::Aspect80_33,
            10 => AspectRatioIdc::Aspect18_11,
            11 => AspectRatioIdc::Aspect15_11,
            12 => AspectRatioIdc::Aspect64_33,
            13 => AspectRatioIdc::Aspect160_99,
            14 => AspectRatioIdc::Aspect4_3,
            15 => AspectRatioIdc::Aspect3_2,
            16 => AspectRatioIdc::Aspect2_1,
            255 => AspectRatioIdc::ExtendedSar,
            _ => AspectRatioIdc::ExtendedSar,
        }
    }
}
