use std::path::Path;

use byteorder::{BigEndian, ReadBytesExt};
use bytes::{Buf, Bytes, BytesMut};
use std::{
    pin::Pin,
    task::{Context, Poll},
};
use tokio::{
    fs::File,
    io::{AsyncRead, AsyncReadExt, BufReader, ReadBuf},
};
use tokio_stream::{Stream, StreamExt};

use crate::tag::FlvTag;
use crate::tag::FlvTagData;
use crate::{data::FlvData, file::FlvFile};
use crate::{header::FlvHeader, tag};

const BUFFER_SIZE: usize = 8 * 1024 * 1024; // 8MB

struct FileStream {
    reader: BufReader<File>,
    buffer: BytesMut,
    // Track accumulated data that needs to be processed
    accumulated: BytesMut,
    // Flag to indicate if we've reached EOF
    reached_eof: bool,
}

impl FileStream {
    async fn new(path: &str) -> std::io::Result<Self> {
        let file = File::open(path).await?;
        // Create a buffer with a reasonable size for reading chunks
        let buffer = BytesMut::with_capacity(BUFFER_SIZE); // 8MB read buffer

        Ok(Self {
            reader: BufReader::new(file),
            buffer,
            accumulated: BytesMut::new(),
            reached_eof: false,
        })
    }
}

impl Stream for FileStream {
    type Item = std::io::Result<Bytes>;

    fn poll_next(self: Pin<&mut Self>, cx: &mut Context<'_>) -> Poll<Option<Self::Item>> {
        let this = self.get_mut();

        // If we've hit EOF and have no more accumulated data, we're done
        if this.reached_eof && this.accumulated.is_empty() {
            return Poll::Ready(None);
        }

        // If we have accumulated data, return it first
        if !this.accumulated.is_empty() {
            let data = this.accumulated.split().freeze();
            return Poll::Ready(Some(Ok(data)));
        }

        // Ensure we have space to read into
        if this.buffer.len() == this.buffer.capacity() {
            this.buffer.reserve(BUFFER_SIZE); // Reserve more space
        }

        let buf_slice = this.buffer.spare_capacity_mut();
        let mut read_buf = ReadBuf::uninit(buf_slice);
        let before_len = read_buf.filled().len();

        match Pin::new(&mut this.reader).poll_read(cx, &mut read_buf) {
            Poll::Ready(Ok(())) => {
                let after_len = read_buf.filled().len();
                let n_bytes = after_len - before_len;

                if n_bytes == 0 {
                    // EOF reached
                    this.reached_eof = true;

                    // If we have data in the buffer, return it
                    if !this.buffer.is_empty() {
                        let data = this.buffer.split().freeze();
                        return Poll::Ready(Some(Ok(data)));
                    }

                    // No more data to read
                    return Poll::Ready(None);
                } else {
                    // Update buffer length to include the new bytes
                    unsafe { this.buffer.set_len(this.buffer.len() + n_bytes) };

                    // Return the filled buffer
                    let data = this.buffer.split().freeze();
                    return Poll::Ready(Some(Ok(data)));
                }
            }
            Poll::Ready(Err(e)) => Poll::Ready(Some(Err(e))),
            Poll::Pending => Poll::Pending,
        }
    }
}

/// Result of parsing an FLV file
pub struct FlvParseResult {
    /// The FLV header
    pub header: FlvHeader,
    /// Number of tags processed
    pub tags_count: u32,
    /// Duration in milliseconds (if available)
    pub duration_ms: Option<u32>,
    /// Last timestamp observed
    pub last_timestamp: u32,
}

pub struct FlvParser;

impl FlvParser {
    /// Parses an FLV file from the given path and returns the header and tags.
    ///
    /// This method efficiently reads the file using buffered I/O and processes
    /// the FLV structure according to the specification.
    pub async fn parse_file(path: &Path) -> Result<u32, Box<dyn std::error::Error>> {
        // Legacy method - call parse_file_with_handler with a no-op handler
        Self::parse_file_with_handler(path, |_| {}).await
    }

    /// Parses an FLV file from the given path, invoking a callback for each item.
    ///
    /// This method efficiently reads the file using buffered I/O and processes
    /// the FLV structure according to the specification, calling the provided handler
    /// function for the header and each tag that is successfully parsed.
    ///
    /// # Arguments
    /// * `path` - Path to the FLV file
    /// * `data_handler` - Closure that gets called for the header and each parsed tag
    ///
    /// # Returns
    /// * `Result<u32, Box<dyn std::error::Error>>` - Result containing tag count or an error
    pub async fn parse_file_with_handler<F>(
        path: &Path,
        mut data_handler: F,
    ) -> Result<u32, Box<dyn std::error::Error>>
    where
        F: FnMut(&FlvData),
    {
        let mut stream = FileStream::new(path.to_str().unwrap()).await?;
        let mut header = None;
        let mut tag_count = 0;
        let mut last_timestamp = 0;

        // Buffer for accumulating data across chunks when needed
        let mut pending_data = BytesMut::new();

        while let Some(chunk) = stream.next().await {
            let mut data = chunk?;

            // Append new data to any pending data from previous chunks
            if !pending_data.is_empty() {
                pending_data.extend_from_slice(&data);
                data = pending_data.split().freeze();
            }

            // Create a cursor to read from the bytes
            let mut cursor = std::io::Cursor::new(data.clone());

            // If header is not parsed yet, parse it
            if header.is_none() {
                match FlvHeader::parse(&mut cursor) {
                    Ok(h) => {
                        header = Some(h.clone());

                        // Call the handler for the header
                        data_handler(&FlvData::Header(h));
                    }
                    Err(e) => {
                        if e.kind() == std::io::ErrorKind::UnexpectedEof {
                            // Save the data and wait for more
                            pending_data.extend_from_slice(&data);
                            continue;
                        } else {
                            return Err(Box::new(e));
                        }
                    }
                }
            }

            // Process any tags in this chunk
            while cursor.has_remaining() {
                let position = cursor.position() as usize;

                // Check if we have at least 4 bytes left for the previous tag size
                if cursor.remaining() < 4 {
                    // Not enough data for the previous tag size, store remaining and continue
                    pending_data.extend_from_slice(&data[position..]);
                    break;
                }

                // Read the previous tag size (4 bytes), we just skip it
                cursor.get_u32();

                // If there is no more data, we need to wait for more
                if !cursor.has_remaining() {
                    break;
                }

                // Try to demux the tag
                match FlvTag::demux(&mut cursor) {
                    Ok(tag) => {
                        tag_count += 1;

                        // Update last timestamp
                        if tag.timestamp_ms > last_timestamp {
                            last_timestamp = tag.timestamp_ms;
                        }

                        // Call the handler with the tag wrapped in FlvData
                        data_handler(&FlvData::Tag(tag));
                    }
                    Err(e) => {
                        match e.kind() {
                            std::io::ErrorKind::UnexpectedEof => {
                                // Not enough data to read the full tag
                                // Store all remaining data from current position and wait for more
                                pending_data.extend_from_slice(&data[position..]);
                                break;
                            }
                            _ => {
                                println!("Error reading tag: {}", e);
                                return Err(Box::new(e));
                            }
                        }
                    }
                }
            }
        }

        match header {
            Some(_) => Ok(tag_count),
            None => Err(Box::new(std::io::Error::new(
                std::io::ErrorKind::UnexpectedEof,
                "Failed to read FLV header",
            ))),
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::tag::FlvTagType;
    use std::time::Instant;

    #[tokio::test]
    async fn test_read_file() -> Result<(), Box<dyn std::error::Error>> {
        let path = Path::new("D:/Downloads/07_47_26-今天能超过10个人吗？.flv");

        // Skip the test if the file doesn't exist
        if !path.exists() {
            println!("Test file not found, skipping test");
            return Ok(());
        }

        // Get file size before parsing
        let file_size = std::fs::metadata(path)?.len();
        let file_size_mb = file_size as f64 / (1024.0 * 1024.0);

        let start = Instant::now(); // Start timer
        let tags_count = FlvParser::parse_file(path).await?;
        let duration = start.elapsed(); // Stop timer

        // Calculate read speed
        let seconds = duration.as_secs() as f64 + duration.subsec_nanos() as f64 * 1e-9;
        let speed_mbps = file_size_mb / seconds;

        println!("Parsed FLV file in {:?}", duration);
        println!("File size: {:.2} MB", file_size_mb);
        println!("Read speed: {:.2} MB/s", speed_mbps);

        println!("Successfully parsed FLV file with {} tags", tags_count);

        Ok(())
    }

    #[tokio::test]
    async fn test_read_file_with_handler() -> Result<(), Box<dyn std::error::Error>> {
        let path = Path::new("D:/Downloads/07_47_26-今天能超过10个人吗？.flv");

        // Skip the test if the file doesn't exist
        if !path.exists() {
            println!("Test file not found, skipping test");
            return Ok(());
        }

        // Counters for different data types
        let mut header_count = 0;
        let mut video_count = 0;
        let mut audio_count = 0;
        let mut script_count = 0;

        let start = Instant::now(); // Start timer

        // Parse with a handler that counts different data types
        let tags_count = FlvParser::parse_file_with_handler(path, |data| match data {
            FlvData::Header(_) => header_count += 1,
            FlvData::Tag(tag) => match tag.tag_type {
                FlvTagType::Video => video_count += 1,
                FlvTagType::Audio => audio_count += 1,
                FlvTagType::ScriptData => script_count += 1,
                _ => {}
            },
            FlvData::EndOfSequence(_) => {}
        })
        .await?;

        let duration = start.elapsed(); // Stop timer

        println!("Parsed FLV file in {:?}", duration);
        println!("Headers: {}", header_count);
        println!("Total tags: {}", tags_count);
        println!("Video tags: {}", video_count);
        println!("Audio tags: {}", audio_count);
        println!("Script tags: {}", script_count);

        Ok(())
    }
}
