mod file;
mod url;

use flv_fix::pipeline::PipelineConfig;
use siphon::downloader::DownloaderConfig;
use std::path::{Path, PathBuf};
use tracing::{error, info};

use crate::utils::progress::ProgressManager;

/// Determine the type of input and process accordingly
pub async fn process_inputs(
    inputs: &[String],
    output_dir: &Path,
    config: PipelineConfig,
    download_config: DownloaderConfig,
    enable_processing: bool,
    name_template: Option<&str>,
    progress_manager: &mut ProgressManager,
) -> Result<(), Box<dyn std::error::Error>> {
    if inputs.is_empty() {
        return Err("No input files or URLs provided".into());
    }

    info!(
        inputs_count = inputs.len(),
        "Starting processing of {} input{}",
        inputs.len(),
        if inputs.len() == 1 { "" } else { "s" }
    );

    // Process each input
    for (index, input) in inputs.iter().enumerate() {
        // Log which file we're processing
        info!(
            input_index = index + 1,
            total_inputs = inputs.len(),
            input = %input,
            "Processing input ({}/{})",
            index + 1,
            inputs.len()
        );

        // Update progress manager if it's not disabled
        progress_manager.set_status(&format!(
            "Processing input ({}/{}) - {}",
            index + 1,
            inputs.len(),
            input
        ));

        // Attempt to parse as a URL first
        if input.starts_with("http://") || input.starts_with("https://") {
            // It's a URL
            url::process_url(
                input,
                output_dir,
                config.clone(),
                download_config.clone(),
                enable_processing,
                name_template,
                progress_manager,
            )
            .await?;
        } else {
            // It's a file path
            let path = PathBuf::from(input);
            if path.exists() && path.is_file() {
                file::process_file(
                    &path,
                    output_dir,
                    config.clone(),
                    enable_processing,
                    progress_manager,
                )
                .await?;
            } else {
                error!(
                    "Input is neither a valid URL nor an existing file: {}",
                    input
                );
                return Err(format!("Invalid input: {}", input).into());
            }
        }
    }

    Ok(())
}
