use flv_fix::writer_task::FlvWriterTask;
use indicatif::{FormattedDuration, MultiProgress, ProgressBar, ProgressStyle};
use std::time::Duration;

use crate::utils::format_duration;

/// A struct that manages multiple progress bars for file operations
#[derive(Clone)]
pub struct ProgressManager {
    multi: MultiProgress,
    main_progress: ProgressBar,
    pub file_progress: Option<ProgressBar>,
    status_progress: ProgressBar,
}

#[allow(dead_code)]
impl ProgressManager {
    /// Creates a new progress manager with a main progress bar
    pub fn new(total_size: Option<u64>) -> Self {
        let multi = MultiProgress::new();

        // Main progress bar (for overall progress)
        let main_progress = match total_size {
            Some(size) => {
                let pb = multi.add(ProgressBar::new(size));
                pb.set_style(
                    ProgressStyle::default_bar()
                        .template("{spinner:.green} [{elapsed_precise}] [{bar:40.cyan/blue}] {bytes}/{total_bytes} ({bytes_per_sec}, {eta})")
                        .unwrap()
                        .progress_chars("#>-")
                );
                pb.set_message("Total progress");
                pb
            }
            None => {
                let pb = multi.add(ProgressBar::new_spinner());
                pb.set_style(
                    ProgressStyle::default_spinner()
                        .template("{spinner:.green} {elapsed_precise} {msg}")
                        .unwrap(),
                );
                pb.set_message("Processing...");
                pb.enable_steady_tick(Duration::from_millis(100));
                pb
            }
        };

        // Status bar for messages
        let status_progress = multi.add(ProgressBar::new_spinner());
        status_progress.set_style(
            ProgressStyle::default_spinner()
                .template("{spinner:.blue} {msg}")
                .unwrap(),
        );
        status_progress.set_message("Initializing...");
        status_progress.enable_steady_tick(Duration::from_millis(100));

        Self {
            multi,
            main_progress,
            file_progress: None,
            status_progress,
        }
    }

    /// Add a file progress bar for the current file being processed
    pub fn add_file_progress(&mut self, filename: &str) -> ProgressBar {
        // Remove the old file progress if it exists
        if let Some(old_pb) = self.file_progress.take() {
            old_pb.finish_and_clear();
        }

        let file_progress = self.multi.add(ProgressBar::new(0));
        file_progress.set_style(
            ProgressStyle::default_bar()
                .template("{spinner:.green} [{msg}] [{bar:40.cyan/blue}] {bytes}/{total_bytes} ({bytes_per_sec})")
                .unwrap()
                .progress_chars("#>-")
        );
        file_progress.set_message(format!("Processing {}", filename));

        self.file_progress = Some(file_progress.clone());
        file_progress
    }

    /// Sets up callbacks on a FlvWriterTask to update the progress bars
    pub fn setup_writer_task_callbacks(&self, writer_task: &mut FlvWriterTask) {
        if let Some(file_progress) = &self.file_progress {
            let file_pb = file_progress.clone();

            // Set up status callback for continuous progress updates
            writer_task.set_status_callback(move |path, size, _rate, duration| {
                if let Some(path) = path {
                    let path_display = path
                        .file_name()
                        .unwrap_or_else(|| path.as_os_str())
                        .to_string_lossy();
                    file_pb.set_length(size);
                    file_pb.set_position(size);

                    // Display video duration prominently in the message
                    file_pb.set_message(format!(
                        "Duration: {} | {}",
                        FormattedDuration(std::time::Duration::from_millis(
                            duration.unwrap_or(0) as u64
                        )),
                        path_display,
                    ));
                }
            });

            // Set up segment open callback
            let status_pb_open = self.status_progress.clone();
            writer_task.set_on_segment_open(move |path, segment_num| {
                let filename = path
                    .file_name()
                    .unwrap_or_else(|| path.as_os_str())
                    .to_string_lossy();
                status_pb_open
                    .set_message(format!("Opened segment #{}: {}", segment_num, filename));
            });

            // Set up segment close callback
            let status_pb_close = self.status_progress.clone();
            writer_task.set_on_segment_close(move |path, segment_num, tags, duration| {
                let filename = path
                    .file_name()
                    .unwrap_or_else(|| path.as_os_str())
                    .to_string_lossy();

                // Format duration if available
                let duration_str = match duration {
                    Some(ms) => format_duration(ms as f64 / 1000.0),
                    None => "unknown duration".to_string(),
                };

                status_pb_close.set_message(format!(
                    "Closed segment #{}: {} ({} tags, {})",
                    segment_num, filename, tags, duration_str
                ));
            });
        }
    }

    /// Updates the main progress bar position
    pub fn update_main_progress(&self, position: u64) {
        if self.main_progress.length().unwrap_or(0) > 0 {
            self.main_progress.set_position(position);
        }
    }

    /// Updates the status message
    pub fn set_status(&self, msg: &str) {
        self.status_progress.set_message(msg.to_string());
    }

    /// Finish all progress bars with a final message
    pub fn finish(&self, msg: &str) {
        self.main_progress.finish_with_message(msg.to_string());
        if let Some(file_progress) = &self.file_progress {
            file_progress.finish();
        }
        self.status_progress.finish_with_message(msg.to_string());
    }

    /// Finish just the file progress bar
    pub fn finish_file(&self, msg: &str) {
        if let Some(file_progress) = &self.file_progress {
            file_progress.finish_with_message(msg.to_string());
        }
    }

    /// Get access to the main progress bar
    pub fn get_main_progress(&self) -> &ProgressBar {
        &self.main_progress
    }

    /// Get access to the status progress bar
    pub fn get_status_progress(&self) -> &ProgressBar {
        &self.status_progress
    }

    /// Get access to the file progress bar if it exists
    pub fn get_file_progress(&self) -> Option<&ProgressBar> {
        self.file_progress.as_ref()
    }
}
