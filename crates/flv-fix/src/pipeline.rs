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
    use crate::context::StreamerContext;
    use flv::writer_async::FlvEncoder;

    use chrono::Local;
    use flv::data::FlvData;
    use flv::header::FlvHeader;
    use flv::parser_async::FlvDecoderStream;
    use futures::StreamExt;
    use futures::sink::SinkExt;
    use std::path::Path;
    use tokio::fs::File;
    use tokio::io::BufWriter;
    use tokio_util::codec::FramedWrite;

    // Helper to initialize tracing for tests
    fn init_tracing() {
        let _ = tracing_subscriber::fmt::fmt()
            .with_max_level(tracing::Level::INFO)
            .with_test_writer() // Write to test output
            .try_init();
    }

    // Define a type alias for the async writer stack
    type AsyncFlvWriter = FramedWrite<BufWriter<tokio::fs::File>, FlvEncoder>;

    #[tokio::test]
    async fn test_process() -> Result<(), Box<dyn std::error::Error>> {
        init_tracing(); // Initialize tracing for logging

        // Source and destination paths
        let input_path = Path::new("D:/test/999/16_02_26-福州~ 主播恋爱脑！！！.flv");
        // let input_path = Path::new("E:/test/2024-12-21_00_06_24.flv");
        let output_dir = input_path.parent().ok_or("Invalid input path")?.join("fix");
        tokio::fs::create_dir_all(&output_dir)
            .await
            .unwrap_or_else(|e| {
                tracing::warn!(error = ?e, "Output directory creation failed or already exists");
            });
        let base_name = input_path
            .file_name()
            .ok_or("Cannot get filename")?
            .to_str()
            .ok_or("Filename not valid UTF-8")?;
        let extension = "flv";

        // Skip if test file doesn't exist
        if !input_path.exists() {
            tracing::info!(path = %input_path.display(), "Test file not found, skipping test");
            return Ok(());
        }

        let start_time = std::time::Instant::now(); // Start timer
        tracing::info!(path = %input_path.display(), "Starting FLV processing pipeline test");

        // Create the context
        let context = StreamerContext::default();

        // Create the pipeline with default configuration
        let pipeline = FlvPipeline::new(context);

        // Start a task to parse the input file using async Decoder
        let file_reader = tokio::io::BufReader::new(tokio::fs::File::open(input_path).await?);
        let decoder_stream = FlvDecoderStream::with_capacity(
            file_reader,
            32 * 1024, // Input buffer capacity
        );

        // Create the input stream for the pipeline
        let input_stream = decoder_stream.boxed();

        // Process the stream through the pipeline
        let processed_stream = pipeline.process(input_stream);

        // Pin the output stream so we can await items from it
        futures::pin_mut!(processed_stream);

        // State for writing output files
        let mut current_writer: Option<AsyncFlvWriter> = None;
        let mut file_counter = 0;
        let mut total_tag_count = 0_u64;
        let mut current_file_tag_count = 0_u64;

        // Process results and write asynchronously
        while let Some(result) = processed_stream.next().await {
            match result {
                Ok(FlvData::Header(header)) => {
                    tracing::debug!("Received Header - starting new file segment");
                    if let Some(mut writer) = current_writer.take() {
                        tracing::debug!(
                            tags = current_file_tag_count,
                            file_num = file_counter,
                            "Closing previous file"
                        );
                        // close() flushes the FramedWrite buffer and the underlying BufWriter
                        writer.close().await?;
                        tracing::info!(
                            tags = current_file_tag_count,
                            file_num = file_counter,
                            "Closed previous file segment"
                        );
                    }

                    // Reset tag count for the new file
                    current_file_tag_count = 0;
                    file_counter += 1;

                    // Create a new file path
                    let timestamp = Local::now().format("%Y%m%d_%H%M%S");
                    let output_path = output_dir.join(format!(
                        "{}_part{}_{}.{}", // Changed naming slightly for clarity
                        base_name.trim_end_matches(".flv"),
                        file_counter,
                        timestamp,
                        extension
                    ));

                    tracing::info!(path = %output_path.display(), "Creating new output file");

                    // Create the file asynchronously
                    let output_file = File::create(&output_path).await?;
                    // Wrap in BufWriter
                    let buf_writer = BufWriter::new(output_file);
                    // Create the FramedWrite sink with the encoder
                    let mut new_writer = FramedWrite::new(buf_writer, FlvEncoder::new());

                    // Write the header to the new file
                    new_writer.send(FlvData::Header(header)).await?;

                    // Store the new writer
                    current_writer = Some(new_writer);
                }
                Ok(FlvData::Tag(tag)) => {
                    // If we somehow don't have a writer (e.g., stream didn't start with Header),
                    // create one now with a default header. This might indicate a pipeline issue.
                    if current_writer.is_none() {
                        tracing::warn!(
                            "Received Tag before Header, creating file with default header"
                        );
                        // Reset tag count for the new file
                        current_file_tag_count = 0;
                        file_counter += 1;

                        let timestamp = Local::now().format("%Y%m%d_%H%M%S");
                        let output_path = output_dir.join(format!(
                            "{}_part{}_{}_DEFHEAD.{}", // Mark as default header
                            base_name.trim_end_matches(".flv"),
                            file_counter,
                            timestamp,
                            extension
                        ));

                        tracing::info!(path = %output_path.display(), "Creating initial output file (default header)");

                        let default_header = FlvHeader::new(true, true); // Assuming reasonable defaults
                        let output_file = File::create(&output_path).await?;
                        let buf_writer = BufWriter::new(output_file);
                        let mut new_writer = FramedWrite::new(buf_writer, FlvEncoder::new());

                        // Write the default header first
                        new_writer.send(FlvData::Header(default_header)).await?;

                        current_writer = Some(new_writer);
                    }

                    // Write the tag to the current writer using send()
                    if let Some(writer) = &mut current_writer {
                        // The send method calls the FlvEncoder internally
                        writer.send(FlvData::Tag(tag)).await?;
                        total_tag_count += 1;
                        current_file_tag_count += 1;

                        // Log progress periodically
                        if total_tag_count % 10000 == 0 {
                            tracing::debug!(tags = total_tag_count, "Processed tags...");
                        }
                    }
                }
                Err(e) => {
                    // Log errors from the pipeline stream
                    tracing::error!(error = ?e, "Error received from pipeline stream");
                    // Depending on the error, you might want to stop or continue
                    // For this test, we'll log and continue, but this might hide issues.
                }
                // Handle other FlvData variants if the enum is extended
                #[allow(unreachable_patterns)]
                Ok(_) => { /* Ignore unknown variants for now */ }
            }
        }

        // Flush and close the final writer after the loop finishes
        if let Some(mut writer) = current_writer.take() {
            tracing::debug!(
                tags = current_file_tag_count,
                file_num = file_counter,
                "Closing final file segment"
            );
            writer.close().await?; // Ensure all buffered data is written
            tracing::info!(
                tags = current_file_tag_count,
                file_num = file_counter,
                "Closed final file segment"
            );
        }

        let elapsed = start_time.elapsed();
        tracing::info!(duration = ?elapsed, "Processing completed");

        tracing::info!(
            total_tags = total_tag_count,
            files_written = file_counter,
            "Pipeline finished processing"
        );

        // Basic assertions (optional, but good for tests)
        assert!(
            file_counter > 0,
            "Expected at least one output file to be created"
        );
        assert!(total_tag_count > 0, "Expected tags to be processed");

        Ok(())
    }
}
