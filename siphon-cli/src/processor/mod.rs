mod file;
mod url;

use std::path::{Path, PathBuf};
use tracing::{error, info};

use crate::{config::ProgramConfig, utils::progress::ProgressManager};

/// Determine the type of input and process accordingly
pub async fn process_inputs(
    inputs: &[String],
    output_dir: &Path,
    config: &ProgramConfig,
    name_template: Option<&str>,
    progress_manager: &mut ProgressManager,
) -> Result<(), Box<dyn std::error::Error>> {
    if inputs.is_empty() {
        return Err("No input files or URLs provided".into());
    }

    let inputs_len = inputs.len();
    info!(
        inputs_count = inputs_len,
        "Starting processing of {} input{}",
        inputs_len,
        if inputs_len == 1 { "" } else { "s" }
    );

    // Preallocate a string builder for status messages to avoid repeated allocations
    let mut status_buffer = String::with_capacity(100);

    // Process each input
    for (index, input) in inputs.iter().enumerate() {
        let input_index = index + 1;

        // Log which file we're processing
        info!(
            input_index = input_index,
            total_inputs = inputs_len,
            input = %input,
            "Processing input ({}/{})",
            input_index,
            inputs_len
        );

        // Update progress manager if it's not disabled - reuse the string buffer
        if !progress_manager.is_disabled() {
            status_buffer.clear();
            status_buffer.push_str("Processing input (");
            status_buffer.push_str(&input_index.to_string());
            status_buffer.push('/');
            status_buffer.push_str(&inputs_len.to_string());
            status_buffer.push_str(") - ");
            status_buffer.push_str(input);
            progress_manager.set_status(&status_buffer);
        }

        // Attempt to parse as a URL first
        if input.starts_with("http://") || input.starts_with("https://") {
            // It's a URL
            url::process_url(
                input,
                output_dir,
                config.clone(),
                name_template,
                progress_manager,
            )
            .await?;
        } else {
            // It's a file path
            let path = PathBuf::from(input);
            if path.exists() && path.is_file() {
                file::process_file(&path, output_dir, config.clone(), progress_manager).await?;
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
