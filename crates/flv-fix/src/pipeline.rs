//! # FLV Processing Pipeline
//!
//! This module implements a processing pipeline for fixing and optimizing FLV (Flash Video) streams.
//! The pipeline consists of multiple operators that can transform, validate, and repair FLV data
//! to ensure proper playability and standards compliance.
//!
//! ## Pipeline Architecture
//!
//! ```
//! Input → Defragment → HeaderCheck → Split → GopSort → TimeConsistency →
//!        TimingRepair → Limit → TimeConsistency2 → ScriptFilter → Output
//! ```
//!
//! Each operator addresses specific issues that can occur in FLV streams:
//!
//! - **Defragment**: Handles fragmented streams by buffering and validating segments
//! - **HeaderCheck**: Ensures streams begin with a valid FLV header
//! - **Split**: Divides content at appropriate points for better playability
//! - **GopSort**: Ensures video tags are properly ordered by GOP (Group of Pictures)
//! - **TimeConsistency**: Maintains consistent timestamps throughout the stream
//! - **TimingRepair**: Fixes timestamp anomalies like negative values or jumps
//! - **Limit**: Enforces file size and duration limits
//! - **ScriptFilter**: Removes or modifies problematic script tags

use crate::context::StreamerContext;
use crate::operators::limit::{self, LimitConfig};
use crate::operators::{
    ContinuityMode, DefragmentOperator, FlvOperator, GopSortOperator, HeaderCheckOperator,
    LimitOperator, RepairStrategy, ScriptFilterOperator, SplitOperator, TimeConsistencyOperator,
    TimingRepairConfig, TimingRepairOperator, defragment, time_consistency,
};
use bytes::buf::Limit;
use flv::data::FlvData;
use flv::error::FlvError;
use futures::FutureExt;
use futures::stream::{Stream, StreamExt};
use std::pin::Pin;
use std::sync::Arc;
use tokio::task::JoinHandle;

/// Type alias for a boxed stream of FLV data with error handling
pub type BoxStream<T> = Pin<Box<dyn Stream<Item = Result<T, FlvError>> + Send>>;

/// Configuration options for the FLV processing pipeline
#[derive(Clone)]
pub struct PipelineConfig {
    /// Whether to filter duplicate tags
    pub duplicate_tag_filtering: bool,

    /// Maximum file size limit in bytes (0 = unlimited)
    pub file_size_limit: u64,

    /// Maximum duration limit in seconds (0 = unlimited)
    pub duration_limit: f32,

    /// Strategy for timestamp repair
    pub repair_strategy: RepairStrategy,

    /// Mode for timeline continuity
    pub continuity_mode: ContinuityMode,

    /// Channel buffer capacity for each stage of the pipeline
    pub channel_buffer_size: usize,

    /// Whether to use adaptive buffer sizing
    pub use_adaptive_buffers: bool,
}

impl Default for PipelineConfig {
    fn default() -> Self {
        Self {
            duplicate_tag_filtering: true,
            file_size_limit: 6 * 1024 * 1024 * 1024, // 6 GB
            duration_limit: 0.0,
            repair_strategy: RepairStrategy::Strict,
            continuity_mode: ContinuityMode::Reset,
            channel_buffer_size: 16, // Default buffer size
            use_adaptive_buffers: false,
        }
    }
}

/// Main pipeline for processing FLV streams
pub struct FlvPipeline {
    context: Arc<StreamerContext>,
    config: PipelineConfig,
}

impl FlvPipeline {
    /// Create a new pipeline with default configuration
    pub fn new(context: StreamerContext) -> Self {
        Self {
            context: Arc::new(context),
            config: PipelineConfig::default(),
        }
    }

    /// Create a new pipeline with custom configuration
    pub fn with_config(context: StreamerContext, config: PipelineConfig) -> Self {
        Self {
            context: Arc::new(context),
            config,
        }
    }

    /// Process an FLV stream through the complete processing pipeline
    pub fn process(&self, input: BoxStream<FlvData>) -> BoxStream<FlvData> {
        let context = Arc::clone(&self.context);
        let config = self.config.clone();

        // Create channels for all operators
        let (defrag_tx, defrag_rx) = kanal::bounded_async(config.channel_buffer_size);
        let (header_check_tx, header_check_rx) = kanal::bounded_async(config.channel_buffer_size);
        let (limit_tx, limit_rx) = kanal::bounded_async(config.channel_buffer_size);
        let (gop_sort_tx, gop_sort_rx) = kanal::bounded_async(config.channel_buffer_size);
        let (script_filter_tx, script_filter_rx) = kanal::bounded_async(config.channel_buffer_size);
        let (timing_repair_tx, timing_repair_rx) = kanal::bounded_async(config.channel_buffer_size);
        let (split_tx, split_rx) = kanal::bounded_async(config.channel_buffer_size);
        let (time_consistency_tx, time_consistency_rx) =
            kanal::bounded_async(config.channel_buffer_size);
        let (time_consistency_2_tx, time_consistency_2_rx) =
            kanal::bounded_async(config.channel_buffer_size);
        let (input_tx, input_rx) = kanal::bounded_async(config.channel_buffer_size);

        // Create all operators
        let mut defrag_operator = DefragmentOperator::new(context.clone());
        let mut header_check_operator = HeaderCheckOperator::new(context.clone());
        let limit_config = LimitConfig {
            max_size_bytes: if config.file_size_limit > 0 {
                Some(config.file_size_limit)
            } else {
                None
            },
            max_duration_ms: if config.duration_limit > 0.0 {
                Some((config.duration_limit * 1000.0) as u32)
            } else {
                None
            },
            split_at_keyframes_only: true,
            use_retrospective_splitting: false,
            on_split: None,
        };
        let mut limit_operator = LimitOperator::with_config(context.clone(), limit_config);
        let mut gop_sort_operator = GopSortOperator::new(context.clone());
        let mut script_filter_operator = ScriptFilterOperator::new(context.clone());
        let mut timing_repair_operator =
            TimingRepairOperator::new(context.clone(), TimingRepairConfig::default());
        let mut split_operator = SplitOperator::new(context.clone());
        let mut time_consistency_operator =
            TimeConsistencyOperator::new(context.clone(), config.continuity_mode);
        let mut time_consistency_2_operator =
            TimeConsistencyOperator::new(context.clone(), config.continuity_mode);

        // Store all task handles
        let mut task_handles: Vec<JoinHandle<()>> = Vec::with_capacity(10);

        // Input conversion task
        task_handles.push(tokio::spawn(async move {
            futures::pin_mut!(input);
            while let Some(result) = input.next().await {
                if input_tx.send(result).await.is_err() {
                    break;
                }
            }
        }));

        // Processing pipeline tasks
        task_handles.push(tokio::spawn(async move {
            defrag_operator.process(input_rx, defrag_tx).await;
        }));
        task_handles.push(tokio::spawn(async move {
            header_check_operator
                .process(defrag_rx, header_check_tx)
                .await;
        }));
        task_handles.push(tokio::spawn(async move {
            split_operator.process(header_check_rx, split_tx).await;
        }));
        task_handles.push(tokio::spawn(async move {
            gop_sort_operator.process(split_rx, gop_sort_tx).await;
        }));
        task_handles.push(tokio::spawn(async move {
            time_consistency_operator
                .process(gop_sort_rx, time_consistency_tx)
                .await;
        }));
        task_handles.push(tokio::spawn(async move {
            timing_repair_operator
                .process(time_consistency_rx, timing_repair_tx)
                .await;
        }));
        task_handles.push(tokio::spawn(async move {
            limit_operator.process(timing_repair_rx, limit_tx).await;
        }));
        task_handles.push(tokio::spawn(async move {
            time_consistency_2_operator
                .process(limit_rx, time_consistency_2_tx)
                .await;
        }));
        task_handles.push(tokio::spawn(async move {
            script_filter_operator
                .process(time_consistency_2_rx, script_filter_tx)
                .await;
        }));

        let output_stream = async_stream::stream! {
            loop {
                match script_filter_rx.recv().await {
                    Ok(result) => match result {
                        Ok(data) => yield Ok(data),
                        Err(e) => yield Err(e),
                    },
                    Err(_) => break, // Channel closed
                }
            }
        };

        Box::pin(output_stream)
    }
}

#[cfg(test)]
/// Tests for the FLV processing pipeline
mod test {
    use super::*;
    use bytes::{Buf, Bytes, BytesMut};
    use chrono::Local;
    use flv::data::FlvData;
    use flv::header::FlvHeader;
    use flv::parser_async::{FlvDecoderStream, FlvParser};
    use flv::tag::{FlvTag, FlvTagType};
    use flv::writer::FlvWriter;
    use futures::StreamExt;
    use std::io::Cursor;
    use std::path::Path;
    use tokio::fs::File;
    use tokio::io::{AsyncReadExt, AsyncSeekExt, BufReader};

    // Helper to initialize tracing for tests
    fn init_tracing() {
        let _ = tracing_subscriber::fmt::try_init();
    }

    #[tokio::test]
    async fn test_process() -> Result<(), Box<dyn std::error::Error>> {
        init_tracing(); // Initialize tracing for logging
        // Source and destination paths
        let input_path = Path::new("D:/test/999/16_02_26-福州~ 主播恋爱脑！！！.flv");
        let output_dir = input_path.parent().unwrap().join("fix");
        std::fs::create_dir_all(&output_dir).unwrap_or_else(|_| {
            println!("Output directory already exists, using it.");
        });
        let base_name = input_path.file_name().unwrap().to_str().unwrap();
        let extension = "flv";

        // Skip if test file doesn't exist
        if !input_path.exists() {
            println!("Test file not found, skipping test");
            return Ok(());
        }

        let mut start_time = std::time::Instant::now(); // Start timer

        // Create the context
        let context = StreamerContext::default();

        // Create the pipeline with default configuration
        let pipeline = FlvPipeline::new(context);

        // Start a task to parse the input file
        let decoder_stream = FlvDecoderStream::with_capacity(
            tokio::io::BufReader::new(tokio::fs::File::open(input_path).await?),
            32 * 1024,
        );

        // Create the input stream
        let input_stream = decoder_stream.boxed();

        // Process the stream
        let processed_stream = pipeline.process(input_stream);

        // Process and write results
        futures::pin_mut!(processed_stream);

        let mut current_writer: Option<FlvWriter<std::fs::File>> = None;
        let mut file_counter = 0;
        let mut total_count = 0;
        let mut current_file_count = 0;

        while let Some(result) = processed_stream.next().await {
            match result {
                Ok(FlvData::Header(header)) => {
                    // Close the current writer if it exists
                    if let Some(mut writer) = current_writer.take() {
                        writer.flush()?;
                        println!("Wrote {} tags to file {}", current_file_count, file_counter);
                        current_file_count = 0;
                    }

                    // Create a new file with timestamp
                    file_counter += 1;
                    let timestamp = Local::now().format("%Y%m%d_%H%M%S");
                    let output_path = output_dir.join(format!(
                        "{}_{}_part{}.{}",
                        base_name, timestamp, file_counter, extension
                    ));

                    println!("Creating new output file: {:?}", output_path);

                    let output_file = std::fs::File::create(&output_path)?;
                    current_writer = Some(FlvWriter::new(output_file, &header)?);
                }
                Ok(FlvData::Tag(tag)) => {
                    // If we don't have a writer, create one with a default header
                    if current_writer.is_none() {
                        file_counter += 1;
                        let timestamp = Local::now().format("%Y%m%d_%H%M%S");
                        let output_path = output_dir.join(format!(
                            "{}_{}_part{}.{}",
                            base_name, timestamp, file_counter, extension
                        ));

                        println!("Creating initial output file: {:?}", output_path);

                        let default_header = FlvHeader::new(true, true);
                        let output_file = std::fs::File::create(&output_path)?;
                        current_writer = Some(FlvWriter::new(output_file, &default_header)?);
                    }

                    // Write the tag to the current writer
                    if let Some(writer) = &mut current_writer {
                        writer.write_tag(tag.tag_type, tag.data, tag.timestamp_ms)?;
                        total_count += 1;
                        current_file_count += 1;
                    }
                }
                Err(e) => eprintln!("Error: {}", e),
                _ => {}
            }
        }

        // Flush and close the final writer
        // if let Some(mut writer) = current_writer {
        //     writer.flush()?;
        //     println!("Wrote {} tags to file {}", current_file_count, file_counter);
        // }

        println!("Processing completed in {:.2?}", start_time.elapsed());

        println!(
            "Processed and wrote {} tags across {} files",
            total_count, file_counter
        );

        Ok(())
    }
}
