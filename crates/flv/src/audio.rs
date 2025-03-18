#[derive(Debug)]
pub enum SoundFormat {
    Pcm = 0,
    Adpcm = 1,
    Mp3 = 2,
    PcmLe = 3,
    Nellymoser16khzMono = 4,
    Nellymoser8khzMono = 5,
    Nellymoser = 6,
    G711A = 7,
    G711Mu = 8,
    Reserved = 9,
    Aac = 10,
    Speex = 11,
    Mp38k = 14,
    DeviceSpecific = 15,
    ExHeader = 16,
}

impl SoundFormat {
    pub fn from_u8(value: u8) -> Result<Self, &'static str> {
        match value {
            0 => Ok(Self::Pcm),
            1 => Ok(Self::Adpcm),
            2 => Ok(Self::Mp3),
            3 => Ok(Self::PcmLe),
            4 => Ok(Self::Nellymoser16khzMono),
            5 => Ok(Self::Nellymoser8khzMono),
            6 => Ok(Self::Nellymoser),
            7 => Ok(Self::G711A),
            8 => Ok(Self::G711Mu),
            9 => Ok(Self::Reserved),
            10 => Ok(Self::Aac),
            11 => Ok(Self::Speex),
            14 => Ok(Self::Mp38k),
            15 => Ok(Self::DeviceSpecific),
            16 => Ok(Self::ExHeader),
            _ => Err("Unknown sound format"),
        }
    }
}