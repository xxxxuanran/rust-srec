use std::error::Error;
use std::fmt;

#[derive(Debug)]
pub enum FlvError {
    IoError(std::io::Error),
    MalformedData(String),
    FragmentedStream,
    InvalidHeader,
    IncompleteData,
    TimestampError(String),
    // Other error variants as needed
}

impl fmt::Display for FlvError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            FlvError::IoError(err) => write!(f, "I/O error: {}", err),
            FlvError::MalformedData(msg) => write!(f, "Malformed FLV data: {}", msg),
            FlvError::FragmentedStream => write!(f, "Fragmented FLV stream detected"),
            FlvError::InvalidHeader => write!(f, "Invalid FLV header"),
            FlvError::IncompleteData => write!(f, "Incomplete FLV data"),
            FlvError::TimestampError(msg) => write!(f, "Timestamp error: {}", msg),
            // Handle other variants
        }
    }
}

impl Error for FlvError {
    fn source(&self) -> Option<&(dyn Error + 'static)> {
        match self {
            FlvError::IoError(err) => Some(err),
            _ => None,
        }
    }
}

impl From<std::io::Error> for FlvError {
    fn from(err: std::io::Error) -> Self {
        FlvError::IoError(err)
    }
}
