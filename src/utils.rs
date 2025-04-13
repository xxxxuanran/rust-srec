use reqwest::Url;
use reqwest::header::{HeaderMap, HeaderName, HeaderValue};
use std::path::Path;
use tokio::fs;
use tracing::info;

// Error types for parsing
#[derive(Debug)]
#[allow(clippy::enum_variant_names)]
pub enum ParseError {
    InvalidFormat,
    InvalidNumber,
    InvalidUnit,
}

impl std::fmt::Display for ParseError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            ParseError::InvalidFormat => write!(f, "Invalid format"),
            ParseError::InvalidNumber => write!(f, "Invalid number"),
            ParseError::InvalidUnit => write!(f, "Invalid unit"),
        }
    }
}

impl std::error::Error for ParseError {}

/// Parse a header string in format "Name: Value" and add it to the HeaderMap
pub fn parse_and_add_header(headers: &mut HeaderMap, header_str: &str) {
    // Find the first colon which separates name and value
    if let Some(colon_pos) = header_str.find(':') {
        let name = header_str[..colon_pos].trim();
        let value = header_str[colon_pos + 1..].trim();

        // Try to create a header name and value
        if let Ok(header_name) = HeaderName::from_bytes(name.as_bytes()) {
            if let Ok(header_value) = HeaderValue::from_str(value) {
                info!("Adding header: {}: {}", name, value);
                headers.insert(header_name, header_value);
                return;
            }
        } else {
            tracing::warn!("Invalid header name: '{}'", name);
        }
    }

    // Log error if header format is invalid
    tracing::warn!(
        "Invalid header format: '{}'. Expected 'Name: Value'",
        header_str
    );
}

/// Parse a collection of header strings and return a HeaderMap
pub fn parse_headers(header_strings: &[String]) -> HeaderMap {
    let mut headers = HeaderMap::new();

    for header_str in header_strings {
        parse_and_add_header(&mut headers, header_str);
    }

    headers
}

/// Function to parse size with units
pub fn parse_size(size_str: &str) -> Result<u64, ParseError> {
    // Trim whitespace and handle case-insensitivity
    let size_str = size_str.trim().to_lowercase();

    // Handle empty string
    if size_str.is_empty() {
        return Err(ParseError::InvalidFormat);
    }

    // Split the numeric part and the unit
    let mut numeric_part = String::new();
    let mut unit_part = String::new();

    for c in size_str.chars() {
        if c.is_ascii_digit() || c == '.' {
            numeric_part.push(c);
        } else {
            unit_part.push(c);
        }
    }

    // Handle cases with no unit (assume bytes)
    if unit_part.is_empty() {
        let bytes = numeric_part
            .parse::<u64>()
            .map_err(|_| ParseError::InvalidNumber)?;
        return Ok(bytes);
    }

    // Parse the numeric part
    let value = numeric_part
        .parse::<f64>()
        .map_err(|_| ParseError::InvalidNumber)?;

    // Parse the unit and convert to bytes
    match unit_part.trim() {
        "b" => Ok(value as u64),
        "kb" => Ok((value * 1024.0) as u64),
        "mb" => Ok((value * 1024.0 * 1024.0) as u64),
        "gb" => Ok((value * 1024.0 * 1024.0 * 1024.0) as u64),
        "tb" => Ok((value * 1024.0 * 1024.0 * 1024.0 * 1024.0) as u64),
        _ => Err(ParseError::InvalidUnit),
    }
}

/// Function to parse time with units
pub fn parse_time(time_str: &str) -> Result<f32, ParseError> {
    // Trim whitespace and handle case-insensitivity
    let time_str = time_str.trim().to_lowercase();

    // Handle empty string
    if time_str.is_empty() {
        return Err(ParseError::InvalidFormat);
    }

    // Try to parse as a simple number (seconds)
    if let Ok(seconds) = time_str.parse::<f32>() {
        return Ok(seconds);
    }

    // Split the numeric part and the unit
    let mut numeric_part = String::new();
    let mut unit_part = String::new();

    for c in time_str.chars() {
        if c.is_ascii_digit() || c == '.' {
            numeric_part.push(c);
        } else {
            unit_part.push(c);
        }
    }

    // Parse the numeric part
    let value = numeric_part
        .parse::<f32>()
        .map_err(|_| ParseError::InvalidNumber)?;

    // Parse the unit and convert to seconds
    match unit_part.trim() {
        "s" => Ok(value),
        "m" => Ok(value * 60.0),
        "h" => Ok(value * 3600.0),
        _ => Err(ParseError::InvalidUnit),
    }
}

/// Convert bytes to a human-readable format
pub fn format_bytes(bytes: u64) -> String {
    const KB: u64 = 1024;
    const MB: u64 = KB * 1024;
    const GB: u64 = MB * 1024;

    if bytes >= GB {
        format!("{:.2} GB", bytes as f64 / GB as f64)
    } else if bytes >= MB {
        format!("{:.2} MB", bytes as f64 / MB as f64)
    } else if bytes >= KB {
        format!("{:.2} KB", bytes as f64 / KB as f64)
    } else {
        format!("{} B", bytes)
    }
}

/// Convert seconds to a human-readable format
pub fn format_duration(seconds: f32) -> String {
    if seconds >= 3600.0 {
        let hours = seconds / 3600.0;
        format!("{:.2}h", hours)
    } else if seconds >= 60.0 {
        let minutes = seconds / 60.0;
        format!("{:.2}m", minutes)
    } else {
        format!("{:.2}s", seconds)
    }
}

/// Expand filename template with placeholders similar to FFmpeg
pub fn expand_filename_template(
    template: &str,
    url: Option<&Url>,
    metadata_title: Option<&str>,
) -> String {
    use chrono::Local;

    let now = Local::now();
    let mut result = String::with_capacity(template.len() * 2);
    let mut chars = template.chars().peekable();

    while let Some(c) = chars.next() {
        if c == '%' {
            if let Some(&next_char) = chars.peek() {
                match next_char {
                    // Date and time placeholders
                    'Y' => {
                        result.push_str(&now.format("%Y").to_string()); // Year (YYYY)
                        chars.next();
                    }
                    'm' => {
                        result.push_str(&now.format("%m").to_string()); // Month (01-12)
                        chars.next();
                    }
                    'd' => {
                        result.push_str(&now.format("%d").to_string()); // Day (01-31)
                        chars.next();
                    }
                    'H' => {
                        result.push_str(&now.format("%H").to_string()); // Hour (00-23)
                        chars.next();
                    }
                    'M' => {
                        result.push_str(&now.format("%M").to_string()); // Minute (00-59)
                        chars.next();
                    }
                    'S' => {
                        result.push_str(&now.format("%S").to_string()); // Second (00-59)
                        chars.next();
                    }

                    // URL-based placeholders
                    'u' => {
                        if let Some(url) = url {
                            // Use the host as placeholder value
                            if let Some(host) = url.host_str() {
                                result.push_str(host);
                            } else {
                                result.push_str("unknown");
                            }
                        } else {
                            result.push_str("local");
                        }
                        chars.next();
                    }
                    'f' => {
                        if let Some(url) = url {
                            // Extract filename from URL path
                            let file_name = url
                                .path_segments()
                                .and_then(|mut segments| segments.next_back())
                                .unwrap_or("download");

                            // Remove extension if present
                            let base_name = match file_name.rfind('.') {
                                Some(pos) => &file_name[..pos],
                                None => file_name,
                            };

                            result.push_str(base_name);
                        } else {
                            result.push_str("file");
                        }
                        chars.next();
                    }

                    // Metadata-based placeholders
                    't' => {
                        if let Some(title) = metadata_title {
                            // Sanitize the title for use in a filename
                            let sanitized = sanitize_filename(title);
                            result.push_str(&sanitized);
                        } else {
                            result.push_str("untitled");
                        }
                        chars.next();
                    }

                    // Literal percent sign
                    '%' => {
                        result.push('%');
                        chars.next();
                    }

                    // Unrecognized placeholder, treat as literal
                    _ => {
                        result.push('%');
                    }
                }
            } else {
                // % at the end of string, treat as literal
                result.push('%');
            }
        } else {
            result.push(c);
        }
    }

    // Sanitize the entire filename to ensure it's valid
    sanitize_filename(&result)
}

/// Sanitize a string for use as a filename
pub fn sanitize_filename(input: &str) -> String {
    // Replace characters that are invalid in filenames
    let invalid_chars = ['<', '>', ':', '"', '/', '\\', '|', '?', '*'];
    let mut result = String::with_capacity(input.len());

    for c in input.chars() {
        if invalid_chars.contains(&c) || c < ' ' {
            result.push('_');
        } else {
            result.push(c);
        }
    }

    // Remove leading and trailing dots and spaces
    let remove_array = ['.', ' '];
    let result = result
        .trim_start_matches(|c| remove_array.contains(&c))
        .trim_end_matches(|c| remove_array.contains(&c))
        .to_string();

    // Use a default name if the result is empty
    if result.is_empty() {
        "file".to_string()
    } else {
        // Truncate to reasonable length if too long
        if result.len() > 200 {
            let mut truncated = result.chars().take(200).collect::<String>();
            truncated.push_str("...");
            truncated
        } else {
            result
        }
    }
}

/// Find FLV files in directory
pub async fn find_flv_files(dir: &Path) -> Result<Vec<std::path::PathBuf>, std::io::Error> {
    let mut files = Vec::new();
    let mut entries = fs::read_dir(dir).await?;

    while let Some(entry) = entries.next_entry().await? {
        let path = entry.path();
        if path.is_file() && path.extension().is_some_and(|ext| ext == "flv") {
            files.push(path);
        }
    }

    Ok(files)
}
