use flv::error::FlvError;
use reqwest::StatusCode;

// Custom error type for download operations
#[derive(Debug, thiserror::Error)]
pub enum DownloadError {
    #[error("HTTP error: {0}")]
    HttpError(#[from] reqwest::Error),

    #[error("Invalid URL: {0}")]
    UrlError(String),

    #[error("Server returned status code {0}")]
    StatusCode(StatusCode),

    #[error("I/O error: {0}")]
    IoError(#[from] std::io::Error),

    #[error("FLV parsing error: {0}")]
    FlvError(#[from] FlvError),

    #[error("Invalid proxy configuration: {0}")]
    ProxyError(String),
}

impl From<DownloadError> for FlvError {
    fn from(err: DownloadError) -> Self {
        FlvError::Io(std::io::Error::new(
            std::io::ErrorKind::Other,
            format!("Download error: {}", err),
        ))
    }
}
