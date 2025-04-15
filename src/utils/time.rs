use crate::error::ParseError;

/// Function to parse time with units
pub fn parse_time(time_str: &str) -> Result<f64, ParseError> {
    // Trim whitespace and handle case-insensitivity
    let time_str = time_str.trim().to_lowercase();

    // Handle empty string
    if time_str.is_empty() {
        return Err(ParseError::InvalidFormat);
    }

    // Try to parse as a simple number (seconds)
    if let Ok(seconds) = time_str.parse::<f64>() {
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
        .parse::<f64>()
        .map_err(|_| ParseError::InvalidNumber)?;

    // Parse the unit and convert to seconds
    match unit_part.trim() {
        "s" => Ok(value),
        "m" => Ok(value * 60.0),
        "h" => Ok(value * 3600.0),
        _ => Err(ParseError::InvalidUnit),
    }
}

/// Convert seconds to a human-readable format
pub fn format_duration(seconds: f64) -> String {
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
