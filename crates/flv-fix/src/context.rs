//! Stream processing context and configuration
//!
//! This module provides the context and configuration structures needed for
//! FLV stream processing. It includes statistics tracking, processing configuration
//! options, and shared context for operators in the processing pipeline.

use std::collections::HashMap;
use std::sync::{Arc, Mutex};
use std::time::Duration;

/// Statistics collected during FLV stream processing
///
/// Tracks various metrics about the processed stream including tag counts,
/// fragmentation information, file characteristics, and codec details.
#[derive(Debug, Default)]
pub struct Statistics {
    /// Total number of FLV tags processed
    pub processed_tags: usize,
    /// Count of detected fragmented segments
    pub fragmented_segments: usize,
    /// Total size of the processed data in bytes
    pub file_size: usize,
    /// Total duration of the stream
    pub duration: Duration,
    /// Count of keyframes in the stream
    pub keyframes: usize,
    /// Whether the stream contains video data
    pub has_video: bool,
    /// Whether the stream contains audio data
    pub has_audio: bool,
    /// Identified video codec (if present)
    pub video_codec: Option<String>,
    /// Identified audio codec (if present)
    pub audio_codec: Option<String>,
}

/// Configuration for FLV stream processing
///
/// Controls various aspects of the processing pipeline behavior including
/// fragmentation handling and validation requirements.
#[derive(Debug)]
pub struct ProcessingConfig {
    /// Minimum size for considering a segment as a fragment (in tags)
    pub min_fragment_size: usize,
    /// Whether keyframes are required for valid segments
    pub require_keyframe: bool,
    /// Whether to allow processing files with missing/empty headers
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

/// Shared context for FLV stream processing operations
///
/// Provides a common context shared across the processing pipeline including
/// the stream name, statistics, configuration, and metadata. This context is used
/// by operators to coordinate their actions and share information.
#[derive(Debug)]
pub struct StreamerContext {
    /// Name of the stream/file being processed
    pub name: String,
    /// Runtime statistics about the processing operation
    pub statistics: Arc<Mutex<Statistics>>,
    /// Processing configuration options
    pub config: ProcessingConfig,
    /// Additional metadata properties
    pub metadata: Arc<Mutex<HashMap<String, String>>>,
}

impl StreamerContext {
    /// Create a new StreamerContext with the specified configuration
    ///
    /// # Arguments
    /// * `config` - The processing configuration to use
    ///
    /// # Returns
    /// A new StreamerContext with the given configuration and default values for other fields
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
