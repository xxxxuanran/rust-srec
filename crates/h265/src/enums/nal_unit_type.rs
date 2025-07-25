/// NAL (Network Abstraction Layer) unit types as defined by ISO/IEC 23008-2 Table 7-1.
#[derive(Debug, Clone, PartialEq, Copy, PartialOrd, Ord, Eq)]
#[repr(u8)]
pub enum NALUnitType {
    /// Coded slice segment of a non-TSA, non-STSA trailing picture
    ///
    /// NAL unit type class: VCL
    TrailN = 0,
    /// Coded slice segment of a non-TSA, non-STSA trailing picture
    ///
    /// NAL unit type class: VCL
    TrailR = 1,
    /// Coded slice segment of a TSA picture
    ///
    /// NAL unit type class: VCL
    TsaN = 2,
    /// Coded slice segment of a TSA picture
    ///
    /// NAL unit type class: VCL
    TsaR = 3,
    /// Coded slice segment of an STSA picture
    ///
    /// NAL unit type class: VCL
    StsaN = 4,
    /// Coded slice segment of an STSA picture
    ///
    /// NAL unit type class: VCL
    StsaR = 5,
    /// Coded slice segment of a RADL picture
    ///
    /// NAL unit type class: VCL
    RadlN = 6,
    /// Coded slice segment of a RADL picture
    ///
    /// NAL unit type class: VCL
    RadlR = 7,
    /// Coded slice segment of a RASL picture
    ///
    /// NAL unit type class: VCL
    RaslN = 8,
    /// Coded slice segment of a RASL picture
    ///
    /// NAL unit type class: VCL
    RaslR = 9,
    /// Reserved non-IRAP SLNR VCL NAL unit types
    ///
    /// NAL unit type class: VCL
    RsvVclN10 = 10,
    /// Reserved non-IRAP sub-layer reference VCL NAL unit types
    ///
    /// NAL unit type class: VCL
    RsvVclR11 = 11,
    /// Reserved non-IRAP SLNR VCL NAL unit types
    ///
    /// NAL unit type class: VCL
    RsvVclN12 = 12,
    /// Reserved non-IRAP sub-layer reference VCL NAL unit types
    ///
    /// NAL unit type class: VCL
    RsvVclR13 = 13,
    /// Reserved non-IRAP SLNR VCL NAL unit types
    ///
    /// NAL unit type class: VCL
    RsvVclN14 = 14,
    /// Reserved non-IRAP sub-layer reference VCL NAL unit types
    ///
    /// NAL unit type class: VCL
    RsvVclR15 = 15,
    /// Coded slice segment of a BLA picture
    ///
    /// NAL unit type class: VCL
    BlaWLp = 16,
    /// Coded slice segment of a BLA picture
    ///
    /// NAL unit type class: VCL
    BlaWRadl = 17,
    /// Coded slice segment of a BLA picture
    ///
    /// NAL unit type class: VCL
    BlaNLp = 18,
    /// Coded slice segment of an IDR picture
    ///
    /// NAL unit type class: VCL
    IdrWRadl = 19,
    /// Coded slice segment of an IDR picture
    ///
    /// NAL unit type class: VCL
    IdrNLp = 20,
    /// Coded slice segment of a CRA picture
    ///
    /// NAL unit type class: VCL
    CraNut = 21,
    /// Reserved IRAP VCL NAL unit types
    ///
    /// NAL unit type class: VCL
    RsvIrapVcl22 = 22,
    /// Reserved IRAP VCL NAL unit types
    ///
    /// NAL unit type class: VCL
    RsvIrapVcl23 = 23,
    /// Reserved non-IRAP VCL NAL unit types
    ///
    /// NAL unit type class: VCL
    RsvVcl24 = 24,
    /// Reserved non-IRAP VCL NAL unit types
    ///
    /// NAL unit type class: VCL
    RsvVcl25 = 25,
    /// Reserved non-IRAP VCL NAL unit types
    ///
    /// NAL unit type class: VCL
    RsvVcl26 = 26,
    /// Reserved non-IRAP VCL NAL unit types
    ///
    /// NAL unit type class: VCL
    RsvVcl27 = 27,
    /// Reserved non-IRAP VCL NAL unit types
    ///
    /// NAL unit type class: VCL
    RsvVcl28 = 28,
    /// Reserved non-IRAP VCL NAL unit types
    ///
    /// NAL unit type class: VCL
    RsvVcl29 = 29,
    /// Reserved non-IRAP VCL NAL unit types
    ///
    /// NAL unit type class: VCL
    RsvVcl30 = 30,
    /// Reserved non-IRAP VCL NAL unit types
    ///
    /// NAL unit type class: VCL
    RsvVcl31 = 31,
    /// Video parameter set
    ///
    /// NAL unit type class: non-VCL
    VpsNut = 32,
    /// Sequence parameter set
    ///
    /// NAL unit type class: non-VCL
    SpsNut = 33,
    /// Picture parameter set
    ///
    /// NAL unit type class: non-VCL
    PpsNut = 34,
    /// Access unit delimiter
    ///
    /// NAL unit type class: non-VCL
    AudNut = 35,
    /// End of sequence
    ///
    /// NAL unit type class: non-VCL
    EosNut = 36,
    /// End of bitstream
    ///
    /// NAL unit type class: non-VCL
    EobNut = 37,
    /// Filler data
    ///
    /// NAL unit type class: non-VCL
    FdNut = 38,
    /// Supplemental enhancement information
    ///
    /// NAL unit type class: non-VCL
    PrefixSeiNut = 39,
    /// Supplemental enhancement information
    ///
    /// NAL unit type class: non-VCL
    SuffixSeiNut = 40,
    /// Reserved
    ///
    /// NAL unit type class: non-VCL
    RsvNvcl41 = 41,
    /// Reserved
    ///
    /// NAL unit type class: non-VCL
    RsvNvcl42 = 42,
    /// Reserved
    ///
    /// NAL unit type class: non-VCL
    RsvNvcl43 = 43,
    /// Reserved
    ///
    /// NAL unit type class: non-VCL
    RsvNvcl44 = 44,
    /// Reserved
    ///
    /// NAL unit type class: non-VCL
    RsvNvcl45 = 45,
    /// Reserved
    ///
    /// NAL unit type class: non-VCL
    RsvNvcl46 = 46,
    /// Reserved
    ///
    /// NAL unit type class: non-VCL
    RsvNvcl47 = 47,
}

impl From<u8> for NALUnitType {
    fn from(value: u8) -> Self {
        match value {
            0 => NALUnitType::TrailN,
            1 => NALUnitType::TrailR,
            2 => NALUnitType::TsaN,
            3 => NALUnitType::TsaR,
            4 => NALUnitType::StsaN,
            5 => NALUnitType::StsaR,
            6 => NALUnitType::RadlN,
            7 => NALUnitType::RadlR,
            8 => NALUnitType::RaslN,
            9 => NALUnitType::RaslR,
            10 => NALUnitType::RsvVclN10,
            11 => NALUnitType::RsvVclR11,
            12 => NALUnitType::RsvVclN12,
            13 => NALUnitType::RsvVclR13,
            14 => NALUnitType::RsvVclN14,
            15 => NALUnitType::RsvVclR15,
            16 => NALUnitType::BlaWLp,
            17 => NALUnitType::BlaWRadl,
            18 => NALUnitType::BlaNLp,
            19 => NALUnitType::IdrWRadl,
            20 => NALUnitType::IdrNLp,
            21 => NALUnitType::CraNut,
            22 => NALUnitType::RsvIrapVcl22,
            23 => NALUnitType::RsvIrapVcl23,
            24 => NALUnitType::RsvVcl24,
            25 => NALUnitType::RsvVcl25,
            26 => NALUnitType::RsvVcl26,
            27 => NALUnitType::RsvVcl27,
            28 => NALUnitType::RsvVcl28,
            29 => NALUnitType::RsvVcl29,
            30 => NALUnitType::RsvVcl30,
            31 => NALUnitType::RsvVcl31,
            32 => NALUnitType::VpsNut,
            33 => NALUnitType::SpsNut,
            34 => NALUnitType::PpsNut,
            35 => NALUnitType::AudNut,
            36 => NALUnitType::EosNut,
            37 => NALUnitType::EobNut,
            38 => NALUnitType::FdNut,
            39 => NALUnitType::PrefixSeiNut,
            40 => NALUnitType::SuffixSeiNut,
            41 => NALUnitType::RsvNvcl41,
            42 => NALUnitType::RsvNvcl42,
            43 => NALUnitType::RsvNvcl43,
            44 => NALUnitType::RsvNvcl44,
            45 => NALUnitType::RsvNvcl45,
            46 => NALUnitType::RsvNvcl46,
            47 => NALUnitType::RsvNvcl47,
            _ => panic!("invalid nal_unit_type: {value}"),
        }
    }
}

impl NALUnitType {
    /// Returns `true` if the NAL unit type class of this NAL unit type is VCL (Video Coding Layer).
    ///
    /// See ISO/IEC 23008-2 - Table 7-1, NAL unit type class column.
    pub fn is_vcl(&self) -> bool {
        (*self as u8) <= 31
    }
}
