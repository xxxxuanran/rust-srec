use flv::data::FlvData;
use flv::parser_async::FlvDecoderStream;
use flv_fix::context::StreamerContext;
use flv_fix::pipeline::{BoxStream, FlvPipeline, PipelineConfig};
use flv_fix::writer_task::FlvWriterTask;
use futures::StreamExt;
use std::path::Path;
use tokio::fs::File;
use tokio::io::BufReader;
use tracing::info;

use crate::utils::format_bytes;

/// Process a single FLV file
pub async fn process_file(
    input_path: &Path,
    output_dir: &Path,
    config: PipelineConfig,
    enable_processing: bool,
) -> Result<(), Box<dyn std::error::Error>> {
    let start_time = std::time::Instant::now();

    // Create output directory if it doesn't exist
    tokio::fs::create_dir_all(output_dir).await?;

    // Create base name for output files
    let base_name = input_path
        .file_stem()
        .ok_or("Invalid filename")?
        .to_string_lossy()
        .to_string();

    info!(
        path = %input_path.display(),
        processing_enabled = enable_processing,
        "Starting to process file"
    );

    // Open the file and create decoder stream
    let file = File::open(input_path).await?;
    let file_reader = BufReader::new(file);
    let file_size = file_reader.get_ref().metadata().await?.len();
    let decoder_stream = FlvDecoderStream::with_capacity(
        file_reader,
        32 * 1024, // Input buffer capacity
    );

    // Create the input stream
    let input_stream: BoxStream<FlvData> = decoder_stream.boxed();

    let processed_stream = if enable_processing {
        // Processing mode: run through the processing pipeline
        info!(
            path = %input_path.display(),
            "Processing pipeline enabled, applying fixes and optimizations"
        );

        // Create streamer context and pipeline
        let context = StreamerContext::default();
        let pipeline = FlvPipeline::with_config(context, config);
        pipeline.process(input_stream)
    } else {
        // Raw mode: bypass the pipeline entirely
        info!(
            path = %input_path.display(),
            "Processing pipeline disabled, outputting raw data"
        );
        input_stream
    };

    // Create writer task and run it
    let mut writer_task = FlvWriterTask::new(output_dir.to_path_buf(), base_name).await?;
    writer_task.run(processed_stream).await?;

    let elapsed = start_time.elapsed();
    let total_tags_written = writer_task.total_tags_written();
    let files_created = writer_task.files_created();

    info!(
        path = %input_path.display(),
        input_size = %format_bytes(file_size),
        duration = ?elapsed,
        processing_enabled = enable_processing,
        tags_processed = total_tags_written,
        files_created = files_created,
        "Processing complete"
    );

    Ok(())
}
