use std::collections::HashMap;
use std::sync::{Arc, Mutex};
use std::time::Duration;

#[derive(Debug, Default)]
pub struct Statistics {
    pub processed_tags: usize,
    pub fragmented_segments: usize,
    pub file_size: usize,
    pub duration: Duration,
    pub keyframes: usize,
    pub has_video: bool,
    pub has_audio: bool,
    pub video_codec: Option<String>,
    pub audio_codec: Option<String>,
}

#[derive(Debug)]
pub struct ProcessingConfig {
    pub min_fragment_size: usize,
    pub require_keyframe: bool,
    pub allow_empty_header: bool,
}

impl Default for ProcessingConfig {
    fn default() -> Self {
        Self {
            min_fragment_size: 10,
            require_keyframe: true,
            allow_empty_header: false,
        }
    }
}

#[derive(Debug)]
pub struct StreamerContext {
    pub name: String,
    pub statistics: Arc<Mutex<Statistics>>,
    pub config: ProcessingConfig,
    pub metadata: Arc<Mutex<HashMap<String, String>>>,
}

impl StreamerContext {
    pub fn new(config: ProcessingConfig) -> Self {
        Self {
            name: "DefaultStreamer".to_string(),
            statistics: Arc::new(Mutex::new(Statistics::default())),
            config,
            metadata: Arc::new(Mutex::new(HashMap::new())),
        }
    }
}

impl Default for StreamerContext {
    fn default() -> Self {
        Self::new(ProcessingConfig::default())
    }
}
