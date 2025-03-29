use thiserror::Error;

#[derive(Error, Debug)]
pub enum FlvError {
    #[error("Invalid FLV header")]
    InvalidHeader,
    #[error("I/O error: {0}")]
    Io(#[from] std::io::Error),
    #[error("Incomplete data provided to decoder")]
    IncompleteData, // Keep this if FramedRead needs it, but decoder uses it less now
    #[error("Error parsing tag data: {0}")]
    TagParseError(String), // More specific error for demux failures
    #[error("Resynchronization failed to find valid tag")]
    ResyncFailed, // Optional: if resync gives up entirely
    #[error("Invalid tag type encountered: {0}")]
    InvalidTagType(u8),
    #[error("Tag data size too large: {0}")]
    TagTooLarge(u32),
}
