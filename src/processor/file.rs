use flv::data::FlvData;
use flv::error::FlvError;
use flv::parser_async::FlvDecoderStream;
use flv_fix::context::StreamerContext;
use flv_fix::pipeline::{BoxStream, FlvPipeline, PipelineConfig};
use flv_fix::writer_task::{FlvWriterTask, WriterError};
use futures::StreamExt;
use indicatif::HumanBytes;
use std::path::Path;
use std::sync::mpsc;
use tokio::fs::File;
use tokio::io::BufReader;
use tracing::info;

use crate::utils::format_bytes;
use crate::utils::progress::ProgressManager;

/// Process a single FLV file
pub async fn process_file(
    input_path: &Path,
    output_dir: &Path,
    config: PipelineConfig,
    enable_processing: bool,
    pb_manager: &mut ProgressManager,
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

    // Use provided progress manager or create a new one

    pb_manager.set_status(&format!("Processing {}", input_path.display()));
    // Create a file-specific progress bar
    let file_name = input_path
        .file_name()
        .unwrap_or_default()
        .to_string_lossy()
        .to_string();
    pb_manager.add_file_progress(&file_name);

    let decoder_stream = FlvDecoderStream::with_capacity(
        file_reader,
        32 * 1024, // Input buffer capacity
    );

    // Create the input stream
    let input_stream: BoxStream<FlvData> = decoder_stream.boxed();

    let mut processed_stream = if enable_processing {
        // Processing mode: run through the processing pipeline
        info!(
            path = %input_path.display(),
            "Processing pipeline enabled, applying fixes and optimizations"
        );
        pb_manager.set_status("Processing with optimizations enabled");

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
        pb_manager.set_status("Processing without optimizations");
        input_stream
    };

    let (sender, receiver) = mpsc::sync_channel::<Result<FlvData, FlvError>>(8);

    // Create reader task
    let reader_handle = tokio::spawn(async move {
        let mut bytes_processed = 0;
        while let Some(result) = processed_stream.next().await {
            // Estimate progress based on tag sizes
            if let Ok(data) = &result {
                match data {
                    FlvData::Header(_) => bytes_processed += 9, // FLV header size
                    FlvData::Tag(tag) => bytes_processed += tag.data.len() as u64 + 11, // Tag data + header
                    FlvData::EndOfSequence(bytes) => bytes_processed += bytes.len() as u64, // End of sequence size
                }
            }
            sender.send(result).unwrap();
        }
        bytes_processed
    });

    let output_dir = output_dir.to_path_buf();

    // Clone progress manager for the writer task
    let progress_clone = pb_manager.clone();

    // Create writer task and run it
    let writer_handle = tokio::task::spawn_blocking(move || {
        let mut writer_task = FlvWriterTask::new(output_dir, base_name)?;

        // Set up progress bar callbacks
        progress_clone.setup_writer_task_callbacks(&mut writer_task);

        writer_task.run(receiver)?;

        Ok::<_, WriterError>((
            writer_task.total_tags_written(),
            writer_task.files_created(),
        ))
    });

    // Wait for both tasks to complete
    let bytes_processed = reader_handle.await?;
    let (total_tags_written, files_created) = writer_handle.await??;

    let elapsed = start_time.elapsed();

    // Finish progress bars with summary
    pb_manager.finish(&format!(
        "Processed {} ({}) in {:?}",
        HumanBytes(bytes_processed),
        total_tags_written,
        elapsed
    ));

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
