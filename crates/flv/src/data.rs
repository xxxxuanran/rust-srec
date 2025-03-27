use std::sync::Arc;

use bytes::Bytes;

use crate::{
    avc::AvcPacket,
    header::FlvHeader,
    hevc::HevcPacket,
    tag::{FlvTagOwned, FlvTag, FlvTagData, FlvUtil},
    video::{self, VideoData, VideoFrameType, VideoTagBody},
};

#[derive(Debug, Clone, PartialEq)]
pub enum FlvDataOwned {
    Header(FlvHeader),
    Tag(FlvTagOwned),
    EndOfSequence(Bytes),
}

#[derive(Debug, Clone, PartialEq)]
pub enum FlvData {
    Header(FlvHeader),
    Tag(FlvTag),
    EndOfSequence(Bytes),
}

impl FlvData {
    pub fn size(&self) -> usize {
        match self {
            FlvData::Header(_) => 9,
            FlvData::Tag(tag) => tag.size(),
            FlvData::EndOfSequence(data) => data.len(),
        }
    }
}

impl FlvDataOwned {
    pub fn timestamp(&self) -> u32 {
        match self {
            FlvDataOwned::Header(_) => 0,
            FlvDataOwned::Tag(tag) => tag.timestamp_ms,
            FlvDataOwned::EndOfSequence(_) => 0,
        }
    }

    pub fn is_end_of_sequence(&self) -> bool {
        matches!(self, FlvDataOwned::EndOfSequence(_))
    }

    pub fn is_key_frame(&self) -> bool {
        match self {
            FlvDataOwned::Header(_) => false,
            FlvDataOwned::Tag(tag) => tag.is_key_frame(),
            FlvDataOwned::EndOfSequence(_) => false,
        }
    }
}
