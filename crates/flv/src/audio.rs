use std::io;

use byteorder::{BigEndian, ReadBytesExt};
use bytes::Bytes;
use bytes_util::BytesCursorExt;

use super::aac::{AacPacket, AacPacketType};

#[derive(Debug, Clone, PartialEq)]
pub enum SoundFormat {
    Pcm = 0,
    AdPcm = 1,
    Mp3 = 2,
    PcmLe = 3,
    Nellymoser16khzMono = 4,
    Nellymoser8khzMono = 5,
    Nellymoser = 6,
    G711ALaw = 7,
    G711MuLaw = 8,
    // New in E-RTMP v2
    ExHeader = 9,
    Aac = 10,
    Speex = 11,
    Mp38k = 14,
    DeviceSpecific = 15,
}

impl TryFrom<u8> for SoundFormat {
    type Error = io::Error;

    fn try_from(value: u8) -> Result<Self, Self::Error> {
        Ok(match value {
            0 => SoundFormat::Pcm,
            1 => SoundFormat::AdPcm,
            2 => SoundFormat::Mp3,
            3 => SoundFormat::PcmLe,
            4 => SoundFormat::Nellymoser16khzMono,
            5 => SoundFormat::Nellymoser8khzMono,
            6 => SoundFormat::Nellymoser,
            7 => SoundFormat::G711ALaw,
            8 => SoundFormat::G711MuLaw,
            9 => SoundFormat::ExHeader,
            10 => SoundFormat::Aac,
            11 => SoundFormat::Speex,
            14 => SoundFormat::Mp38k,
            15 => SoundFormat::DeviceSpecific,
            _ => {
                return Err(io::Error::new(
                    io::ErrorKind::InvalidData,
                    format!("Invalid sound format: {}", value),
                ));
            }
        })
    }
}

#[derive(Debug, Clone, PartialEq)]
pub enum SoundRate {
    Hz5512 = 0,
    Hz11025 = 1,
    Hz22050 = 2,
    Hz44100 = 3,
    // New in E-RTMP v2
    Hz48000 = 4,
}

impl TryFrom<u8> for SoundRate {
    type Error = io::Error;

    fn try_from(value: u8) -> Result<Self, Self::Error> {
        Ok(match value {
            0 => SoundRate::Hz5512,
            1 => SoundRate::Hz11025,
            2 => SoundRate::Hz22050,
            3 => SoundRate::Hz44100,
            4 => SoundRate::Hz48000,
            _ => {
                return Err(io::Error::new(
                    io::ErrorKind::InvalidData,
                    format!("Invalid sound rate: {}", value),
                ));
            }
        })
    }
}

// Representation of sound size in Audio Data in FLV
#[derive(Debug, Clone, PartialEq)]
pub enum SoundSize {
    Bits8 = 0,
    Bits16 = 1,
    // New in E-RTMP v2
    Bits24 = 2,
}
impl TryFrom<u8> for SoundSize {
    type Error = io::Error;

    fn try_from(value: u8) -> Result<Self, Self::Error> {
        Ok(match value {
            0 => SoundSize::Bits8,
            1 => SoundSize::Bits16,
            2 => SoundSize::Bits24,
            _ => {
                return Err(io::Error::new(
                    io::ErrorKind::InvalidData,
                    format!("Invalid sound size: {}", value),
                ));
            }
        })
    }
}

#[derive(Debug, Clone, PartialEq)]
pub enum SoundType {
    Mono = 0,
    Stereo = 1,
}

impl TryFrom<u8> for SoundType {
    type Error = io::Error;

    fn try_from(value: u8) -> Result<Self, Self::Error> {
        Ok(match value {
            0 => SoundType::Mono,
            1 => SoundType::Stereo,
            _ => {
                return Err(io::Error::new(
                    io::ErrorKind::InvalidData,
                    format!("Invalid sound type: {}", value),
                ));
            }
        })
    }
}

#[derive(Debug, Clone, PartialEq)]
struct AudioLegacyPacket {
    // bit 3-2
    sound_rate: SoundRate,
    // bit 1
    sound_size: SoundSize,
    // bit 0
    sound_type: SoundType,
}

impl AudioLegacyPacket {
    fn new(sound_rate: SoundRate, sound_size: SoundSize, sound_type: SoundType) -> Self {
        AudioLegacyPacket {
            sound_rate,
            sound_size,
            sound_type,
        }
    }

    fn from_byte(byte: u8) -> Self {
        let sound_rate = SoundRate::try_from((byte >> 2) & 0x03).unwrap();
        let sound_size = SoundSize::try_from((byte >> 1) & 0x01).unwrap();
        let sound_type = SoundType::try_from(byte & 0x01).unwrap();

        AudioLegacyPacket::new(sound_rate, sound_size, sound_type)
    }
}

#[derive(Debug, Clone, PartialEq)]
pub struct AudioHeader {
    // sound format
    sound_format: SoundFormat,
    packet: AudioPacket,
}

// Representation of audio data in FLV
#[derive(Debug, Clone, PartialEq)]
pub struct AudioData {
    header: AudioHeader,
    // Body
    body: AudioDataBody,
}

#[derive(Debug, Clone, PartialEq)]
pub enum AudioDataBody {
    // Usually the body of an audio tag is a aac packet
    Aac(AacPacket),
    /// Some other audio format we don't know how to parse
    Unknown {
        data: Bytes,
    },
}

#[derive(Debug, Clone, PartialEq)]
pub enum AudioPacket {
    // Legacy audio packet, this is the default
    Legacy(AudioLegacyPacket),
    // New in E-RTMP v2, Audio Packet Type
    AudioPacketType(AudioPacketType),
}

// New in E-RTMP v2, Audio Packet Type
#[derive(Debug, Clone, PartialEq)]
pub enum AudioPacketType {
    SequenceStart = 0,
    CodecFrames = 1,
    SequenceEnd = 2,
    MultichannelConfig = 4,
    Multitrack = 5,
    ModEx = 7,
}

impl TryFrom<u8> for AudioPacketType {
    type Error = io::Error;

    fn try_from(value: u8) -> Result<Self, Self::Error> {
        Ok(match value {
            0 => AudioPacketType::SequenceStart,
            1 => AudioPacketType::CodecFrames,
            2 => AudioPacketType::SequenceEnd,
            4 => AudioPacketType::MultichannelConfig,
            5 => AudioPacketType::Multitrack,
            7 => AudioPacketType::ModEx,
            _ => {
                return Err(io::Error::new(
                    io::ErrorKind::InvalidData,
                    format!("Invalid audio packet type: {}", value),
                ));
            }
        })
    }
}

#[derive(Debug, Clone, PartialEq)]
pub enum AudioPacketModExType {
    TimestampOffsetNano = 0,
}

impl TryFrom<u8> for AudioPacketModExType {
    type Error = io::Error;

    fn try_from(value: u8) -> Result<Self, Self::Error> {
        Ok(match value {
            0 => AudioPacketModExType::TimestampOffsetNano,
            _ => {
                return Err(io::Error::new(
                    io::ErrorKind::InvalidData,
                    format!("Invalid audio packet modex type: {}", value),
                ));
            }
        })
    }
}

#[derive(Debug, Clone, PartialEq)]
// New in E-RTMP v2, Audio FourCC
pub enum AudioFourCC {
    Ac3,
    Eac3,
    Opus,
    Mp3,
    Flac,
    Aac,
}

impl AudioFourCC {
    pub fn from_u32(value: u32) -> Result<Self, io::Error> {
        Ok(match value {
            0x00000000 => AudioFourCC::Ac3,
            0x00000001 => AudioFourCC::Eac3,
            0x00000002 => AudioFourCC::Opus,
            0x00000003 => AudioFourCC::Mp3,
            0x00000004 => AudioFourCC::Flac,
            0x00000005 => AudioFourCC::Aac,
            _ => {
                return Err(io::Error::new(
                    io::ErrorKind::InvalidData,
                    format!("Invalid audio fourcc: {}", value),
                ));
            }
        })
    }

    pub fn as_bytes(&self) -> &'static [u8] {
        match self {
            AudioFourCC::Ac3 => b"ac3\0",
            AudioFourCC::Eac3 => b"eac3",
            AudioFourCC::Opus => b"Opus",
            AudioFourCC::Mp3 => b".mp3",
            AudioFourCC::Flac => b"flac",
            AudioFourCC::Aac => b"aac\0",
        }
    }
}

#[derive(Debug, Clone, PartialEq)]
pub enum AvMultitrackType {
    OneTrack = 0,
    ManyTracks = 1,
    ManyTracksManyCodecs = 2,
}

impl TryFrom<u8> for AvMultitrackType {
    type Error = io::Error;

    fn try_from(value: u8) -> Result<Self, Self::Error> {
        Ok(match value {
            0 => AvMultitrackType::OneTrack,
            1 => AvMultitrackType::ManyTracks,
            2 => AvMultitrackType::ManyTracksManyCodecs,
            _ => {
                return Err(io::Error::new(
                    io::ErrorKind::InvalidData,
                    format!("Invalid av multitrack type: {}", value),
                ));
            }
        })
    }
}

impl AudioData {
    pub fn parse(reader: &mut io::Cursor<Bytes>, body_size: usize) -> io::Result<Self> {
        let start = reader.position() as usize;

        // Read the first byte to get the sound format
        let sound_format_byte = reader.read_u8()?;

        // Parse the sound format (bits 7-4)
        let sound_format = SoundFormat::try_from(sound_format_byte >> 4)?;

        let audio_packet = match sound_format {
            // New in E-RTMP v2, new header
            SoundFormat::ExHeader => {
                // Switch to the new fourcc mode
                let audio_packet_type = AudioPacketType::try_from(sound_format_byte & 0x0F)?;

                match audio_packet_type {
                    AudioPacketType::ModEx => {
                        // Process ModEx packets in a loop until we get a non-ModEx packet type
                        let mut final_audio_packet_type = audio_packet_type;
                        let mut audio_timestamp_nano_offset = 0u32;

                        loop {
                            // Determine the size of the packet ModEx data (ranging from 1 to 256 bytes)
                            let mod_ex_data_size = (reader.read_u8()? as usize) + 1;

                            // If maximum 8-bit size is not sufficient, use a 16-bit value
                            let mod_ex_data_size = if mod_ex_data_size == 256 {
                                (reader.read_u16::<BigEndian>()? as usize) + 1
                            } else {
                                mod_ex_data_size
                            };

                            // Fetch the packet ModEx data based on its determined size
                            let mut mod_ex_data = reader.extract_bytes(mod_ex_data_size)?;

                            // Fetch the AudioPacketModExType
                            let next_byte = reader.read_u8()?;
                            let audio_packet_mod_ex_type =
                                AudioPacketModExType::try_from(next_byte >> 4)?;

                            // Update audioPacketType for next iteration or final result
                            let next_audio_packet_type =
                                AudioPacketType::try_from(next_byte & 0x0F)?;

                            // Process specific ModEx types
                            if audio_packet_mod_ex_type == AudioPacketModExType::TimestampOffsetNano
                            {
                                // This enhances RTMP timescale accuracy without altering core RTMP timestamps
                                if mod_ex_data.len() >= 3 {
                                    audio_timestamp_nano_offset = ((mod_ex_data[0] as u32) << 16)
                                        | ((mod_ex_data[1] as u32) << 8)
                                        | (mod_ex_data[2] as u32);
                                    // Note: The audio_timestamp_nano_offset could be stored in the AudioData struct
                                    // and used for precise timing calculations
                                }
                            }

                            // Break the loop if the next packet type is not ModEx
                            if next_audio_packet_type != AudioPacketType::ModEx {
                                final_audio_packet_type = next_audio_packet_type;
                                break;
                            }
                        }

                        // Continue with the final non-ModEx audio packet type
                        let mut is_audio_multitrack = false;
                        let mut audio_multitrack_type = AvMultitrackType::OneTrack;
                        let mut audio_four_cc = None;

                        if final_audio_packet_type == AudioPacketType::Multitrack {
                            is_audio_multitrack = true;

                            // Read the multitrack type
                            let multitrack_type_byte = reader.read_u8()?;
                            audio_multitrack_type =
                                AvMultitrackType::try_from(multitrack_type_byte & 0x0F)?;

                            // Fetch AudioPacketType for all audio tracks in the audio message
                            let packet_type_byte = reader.read_u8()?;
                            let new_packet_type =
                                AudioPacketType::try_from(packet_type_byte & 0x0F)?;

                            // Make sure it's not multitrack again
                            if new_packet_type == AudioPacketType::Multitrack {
                                return Err(io::Error::new(
                                    io::ErrorKind::InvalidData,
                                    "Nested multitrack is not allowed",
                                ));
                            }

                            final_audio_packet_type = new_packet_type;

                            if audio_multitrack_type != AvMultitrackType::ManyTracksManyCodecs {
                                // The tracks are encoded with the same codec, read the FOURCC for them
                                let four_cc = reader.read_u32::<BigEndian>()?;
                                audio_four_cc = Some(AudioFourCC::from_u32(four_cc)?);
                            }
                        } else {
                            // Not multitrack, read the FOURCC
                            let four_cc = reader.read_u32::<BigEndian>()?;
                            audio_four_cc = Some(AudioFourCC::from_u32(four_cc)?);
                        }

                        // Now we're ready to process the audio body based on final_audio_packet_type
                        // For now, we'll just return a placeholder

                        AudioPacket::AudioPacketType(final_audio_packet_type)
                    }
                    // Other audio packet types, not implemented yet
                    other => AudioPacket::AudioPacketType(other),
                }
            }
            // Legacy audio packet
            _ => AudioPacket::Legacy(AudioLegacyPacket::from_byte(sound_format_byte)),
        };

        // read the body
        let body = match sound_format {
            SoundFormat::Aac => {
                // Read the AAC packet type
                let aac_packet_type = AacPacketType::try_from(reader.read_u8()?)?;
                AudioDataBody::Aac(AacPacket::new(
                    aac_packet_type,
                    AudioData::read_remaining(reader, body_size)?,
                ))
            }
            _ => AudioDataBody::Unknown {
                data: AudioData::read_remaining(reader, body_size)?,
            },
        };

        // flv audio header
        let audio_header = AudioHeader {
            sound_format: sound_format,
            packet: audio_packet,
        };

        Ok(AudioData {
            header: audio_header,
            body,
        })
    }

    fn read_remaining(reader: &mut io::Cursor<Bytes>, size: usize) -> io::Result<Bytes> {
        Ok(if size == 0 {
            // if size is 0, read until the end of the stream
            reader.extract_remaining()
        } else {
            // read a fixed size
            reader.extract_bytes(size)?
        })
    }
}

#[cfg(test)]
mod tests {
    use std::io::Cursor;

    use bytes::{BufMut, BytesMut};

    use crate::aac;

    use super::*;

    #[test]
    fn test_parse_aac_audio_packet() {
        let mut reader = io::Cursor::new(Bytes::from(vec![0b10101101, 0b00000000, 1, 2, 3]));
        let audio_data = AudioData::parse(&mut reader, 0).unwrap();

        assert_eq!(audio_data.header.sound_format, SoundFormat::Aac);
        match &audio_data.header.packet {
            AudioPacket::Legacy(legacy) => {
                assert_eq!(legacy.sound_rate, SoundRate::Hz44100);
                assert_eq!(legacy.sound_size, SoundSize::Bits8);
                assert_eq!(legacy.sound_type, SoundType::Stereo);
            }
            _ => panic!("Expected Legacy packet"),
        }
        // match &audio_data.body {
        //     AudioDataBody::Aac(aac) => {
        //         assert_eq!(aac.aac_packet_type, AacPacketType::SequenceHeader);
        //         assert_eq!(aac.data.len(), 2);
        //         assert_eq!(aac.data[0], 0x12);
        //         assert_eq!(aac.data[1], 0x34);
        //     }
        //     _ => panic!("Expected AAC packet"),
        // }
    }

    #[test]
    fn test_parse_unknown_audio_format() {
        let mut bytes = BytesMut::new();
        // MP3 sound format (2 << 4) with mono (0) and Hz11025 (2 << 2) and 16-bit samples (1 << 1)
        bytes.put_u8(0x26); // 0010 0110
        // Some MP3 data
        bytes.put_u8(0xAB);
        bytes.put_u8(0xCD);

        let mut cursor = Cursor::new(bytes.freeze());
        let audio_data = AudioData::parse(&mut cursor, 0).unwrap();

        assert_eq!(audio_data.header.sound_format, SoundFormat::Mp3);
        match &audio_data.header.packet {
            AudioPacket::Legacy(legacy) => {
                assert_eq!(legacy.sound_rate, SoundRate::Hz11025);
                assert_eq!(legacy.sound_size, SoundSize::Bits16);
                assert_eq!(legacy.sound_type, SoundType::Mono);
            }
            _ => panic!("Expected Legacy packet"),
        }
        match &audio_data.body {
            AudioDataBody::Unknown { data } => {
                assert_eq!(data.len(), 2);
                assert_eq!(data[0], 0xAB);
                assert_eq!(data[1], 0xCD);
            }
            _ => panic!("Expected Unknown packet"),
        }
    }

    #[test]
    fn test_read_remaining_with_size() {
        let mut bytes = BytesMut::new();
        bytes.put_u8(0x01);
        bytes.put_u8(0x02);
        bytes.put_u8(0x03);
        bytes.put_u8(0x04);

        let mut cursor = Cursor::new(bytes.freeze());
        let result = AudioData::read_remaining(&mut cursor, 2).unwrap();

        assert_eq!(result.len(), 2);
        assert_eq!(result[0], 0x01);
        assert_eq!(result[1], 0x02);
        assert_eq!(cursor.position(), 2);
    }

    #[test]
    fn test_read_remaining_without_size() {
        let mut bytes = BytesMut::new();
        bytes.put_u8(0x01);
        bytes.put_u8(0x02);
        bytes.put_u8(0x03);

        let mut cursor = Cursor::new(bytes.freeze());
        let result = AudioData::read_remaining(&mut cursor, 0).unwrap();

        assert_eq!(result.len(), 3);
        assert_eq!(result[0], 0x01);
        assert_eq!(result[1], 0x02);
        assert_eq!(result[2], 0x03);
        assert_eq!(cursor.position(), 3);
    }
}
