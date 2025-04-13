pub mod file;
pub mod url;

use flv_fix::pipeline::PipelineConfig;
use siphon::downloader::DownloaderConfig;
use std::path::{Path, PathBuf};
use tracing::{error, info};

/// Process multiple input paths (files, directories, or URLs)
pub async fn process_inputs(
    inputs: &[String],
    output_dir: &Path,
    config: PipelineConfig,
    download_config: DownloaderConfig,
    enable_processing: bool,
    name_template: Option<&str>,
) -> Result<(), Box<dyn std::error::Error>> {
    let mut files_to_process = Vec::new();
    let mut urls_to_process = Vec::new();

    // Separate files/directories from URLs
    for input in inputs {
        // Check if the input looks like a URL
        if input.starts_with("http://") || input.starts_with("https://") {
            urls_to_process.push(input.clone());
            continue;
        }

        // Process as file or directory
        let path = PathBuf::from(input);
        if path.is_file() && path.extension().is_some_and(|ext| ext == "flv") {
            files_to_process.push(path);
        } else if path.is_dir() {
            let flv_files = crate::utils::find_flv_files(&path).await?;
            files_to_process.extend(flv_files);
        } else {
            error!("Input {} is not a valid FLV file, directory, or URL", input);
        }
    }

    // Log what we found
    info!(
        "Found {} local FLV files to process",
        files_to_process.len()
    );
    info!(
        "Found {} URLs to download and process",
        urls_to_process.len()
    );

    if files_to_process.is_empty() && urls_to_process.is_empty() {
        error!("No FLV files or URLs found in the specified inputs");
        return Err("No FLV files or URLs found".into());
    }

    // Process local files
    for file in files_to_process {
        if let Err(e) =
            file::process_file(&file, output_dir, config.clone(), enable_processing).await
        {
            error!(
                file = %file.display(),
                error = ?e,
                "Failed to process file"
            );
        }
    }

    // Process URLs
    for url in urls_to_process {
        if let Err(e) = url::process_url(
            &url,
            output_dir,
            config.clone(),
            download_config.clone(),
            enable_processing,
            name_template,
        )
        .await
        {
            error!(
                url = %url,
                error = ?e,
                "Failed to process URL"
            );
            return Err(e);
        }
    }

    Ok(())
}
