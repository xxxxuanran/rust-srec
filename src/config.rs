use flv_fix::PipelineConfig;
use siphon::DownloaderConfig;

#[derive(Debug, Clone)]
pub struct ProgramConfig {
    pub pipeline_config: PipelineConfig,
    pub download_config: Option<DownloaderConfig>,
    pub channel_size: usize,
    pub enable_processing: bool,
}
