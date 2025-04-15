// Export utility functions
pub use self::headers::parse_headers;
pub use self::size::format_bytes;
pub use self::size::parse_size;
pub use self::template::expand_filename_template;
pub use self::time::format_duration;
pub use self::time::parse_time;
pub mod progress;

// Module declarations
mod headers;
mod size;
mod template;
mod time;
