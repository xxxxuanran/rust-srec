use mesio_engine::hls;
use thiserror::Error;

#[derive(Debug, Error)]
pub enum AppError {
    // #[error("Configuration error: {0}")]
    // Config(String),
    #[error("I/O error: {0}")]
    Io(#[from] std::io::Error),

    #[error("Pipeline error: {0}")]
    Pipeline(#[from] pipeline_common::PipelineError),

    #[error("FLV processing error: {0}")]
    Flv(#[from] flv::error::FlvError),

    #[error("FLV fix error: {0}")]
    FlvFix(#[from] flv_fix::ScriptModifierError),

    #[error("HLS processing error: {0}")]
    Hls(#[from] hls::HlsDownloaderError),

    #[error("Download error: {0}")]
    Download(#[from] mesio_engine::DownloadError),

    #[error("Invalid input: {0}")]
    InvalidInput(String),

    #[error("Parse error: {0}")]
    ParseError(String),

    #[error("Initialization failed: {0}")]
    Initialization(String),

    #[error("Processor error: {0}")]
    Processor(#[from] Box<dyn std::error::Error>),
}
