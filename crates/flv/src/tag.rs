use std::io;

#[derive(Debug, Clone, PartialEq)]
pub enum TagType {
    Audio = 8,
    Video = 9,
    Script = 18,
}



// impl TryFrom<u8> for TagType {
//     type Error = io::Error;
    
//     fn try_from(value: u8) -> Result<Self, Self::Error> {
//         match value {
//             8 => Ok(TagType::Audio),
//             9 => Ok(TagType::Video),
//             18 => Ok(TagType::Script),
//             _ => Err(io::Error::new(
//                 io::ErrorKind::InvalidData,
//                 format!("Invalid tag type: {}", value),
//             )),
//         }
//     }
// }
