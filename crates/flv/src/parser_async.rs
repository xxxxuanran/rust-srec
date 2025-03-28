use crate::data::FlvData;
use crate::error::FlvError;
use crate::header::FlvHeader;
use crate::tag::FlvTag;
use bytes::{Buf, BytesMut};
use futures::{Stream, StreamExt};
use std::{
    io::Cursor,
    path::Path,
    pin::Pin,
    task::{Context, Poll},
};
use tokio::io::AsyncRead;
use tokio_util::codec::{Decoder, FramedRead};

const BUFFER_SIZE: usize = 4 * 1024; // 4 KB buffer size

/// An FLV format decoder that implements Tokio's Decoder trait
pub struct FlvDecoder {
    header_parsed: bool,
    prev_tag_size_read: bool,
}

impl FlvDecoder {
    pub fn new() -> Self {
        Self {
            header_parsed: false,
            prev_tag_size_read: true, // Start with true as the first item is the header
        }
    }
}

impl Decoder for FlvDecoder {
    type Item = FlvData;
    type Error = FlvError;

    fn decode(&mut self, src: &mut BytesMut) -> Result<Option<Self::Item>, Self::Error> {
        // First parse the header if not already done
        if !self.header_parsed {
            if src.len() < 9 {
                // Not enough data yet
                src.reserve(9 - src.len());
                return Ok(None);
            }

            let mut cursor = Cursor::new(&src[..9]);
            match FlvHeader::parse(&mut cursor) {
                Ok(header) => {
                    // Consume the header bytes from the buffer
                    src.advance(9);
                    self.header_parsed = true;
                    return Ok(Some(FlvData::Header(header)));
                }
                Err(e) => {
                    eprintln!("Error parsing FLV header: {:?}", e);
                    return Err(FlvError::InvalidHeader);
                }
            }
        }

        // Handle the prev_tag_size
        if !self.prev_tag_size_read {
            if src.len() < 4 {
                // We try to reserve space for the previous tag size + the tag header
                src.reserve(11 + 4 - src.len());
                return Ok(None);
            }
            // Skip the previous tag size
            src.advance(4);
            self.prev_tag_size_read = true;
        }

        // Check if we have enough for the tag header (11 bytes)
        if src.len() < 11 {
            src.reserve(11 - src.len());
            return Ok(None);
        }

        // Validate the tag type first
        let tag_type = src[0] & 0x1F; // Mask to get the tag type (lower 5 bits)
        if tag_type != 8 && tag_type != 9 && tag_type != 18 {
            eprintln!("Warning: Found invalid tag type: {}", tag_type);
            // Try to resync by finding the next valid tag
            // For now, we'll just skip this byte
            src.advance(1);
            return Ok(None);
        }

        // Read tag data size from bytes 1-3
        let data_size = ((src[1] as u32) << 16) | ((src[2] as u32) << 8) | (src[3] as u32);

        // Validate data_size to prevent unreasonable allocations
        if data_size > 16_777_215 {
            // Max reasonable size for FLV tag
            eprintln!("Warning: Unusually large tag data size: {}", data_size);
            // Skip this tag header and try to resync
            src.advance(11);
            self.prev_tag_size_read = false;
            return Ok(None);
        }

        let total_needed = 11 + data_size as usize;

        // Check if we have the full tag
        if src.len() < total_needed {
            // Reserve enough space for the tag data
            src.reserve(total_needed - src.len());
            return Ok(None);
        }

        // Create a cursor over the tag data
        let tag_data = src.split_to(total_needed);
        let mut cursor = Cursor::new(tag_data.freeze());

        match FlvTag::demux(&mut cursor) {
            Ok(tag) => {
                self.prev_tag_size_read = false; // Need to read next tag's prev_tag_size
                Ok(Some(FlvData::Tag(tag)))
            }
            Err(e) => {
                eprintln!(
                    "Error parsing FLV tag: {:?}, tag data size: {}",
                    e, data_size
                );

                // Instead of returning an error, try to recover
                Err(FlvError::IncompleteData)
            }
        }
    }
}

pub struct FlvDecoderStream<R> {
    framed: FramedRead<R, FlvDecoder>,
}

impl<R: AsyncRead + Unpin> FlvDecoderStream<R> {
    pub fn new(reader: R) -> Self {
        Self {
            framed: FramedRead::with_capacity(reader, FlvDecoder::new(), BUFFER_SIZE),
        }
    }

    pub fn with_capacity(reader: R, capacity: usize) -> Self {
        Self {
            framed: FramedRead::with_capacity(reader, FlvDecoder::new(), capacity),
        }
    }
}

impl<R: AsyncRead + Unpin> Stream for FlvDecoderStream<R> {
    type Item = Result<FlvData, FlvError>;

    fn poll_next(mut self: Pin<&mut Self>, cx: &mut Context<'_>) -> Poll<Option<Self::Item>> {
        self.framed.poll_next_unpin(cx)
    }
}

pub struct FlvParser;

impl FlvParser {
    /// Create a stream using Tokio's Decoder interface
    pub async fn create_decoder_stream(
        path: &Path,
    ) -> Result<impl Stream<Item = Result<FlvData, FlvError>>, std::io::Error> {
        let file = tokio::fs::File::open(path).await?;
        let reader = tokio::io::BufReader::new(file);

        Ok(FlvDecoderStream::new(reader))
    }
}

#[cfg(test)]
mod tests {

    use super::*;
    use crate::header::FlvHeader;
    use crate::tag::{FlvTag, FlvUtil};
    use bytes::BytesMut;
    use futures::TryStreamExt;
    use std::collections::HashMap;
    use std::io::Cursor;
    use std::time::Instant;

    #[test]
    fn test_flv_decoder() {
        let data = vec![
            0x46, 0x4C, 0x56, // "FLV" signature
            0x01, // version
            0x05, // flags (audio and video)
            0x00, 0x00, 0x00, 0x09, // data offset (9 bytes)
            // FLV tag header (11 bytes)
            0x09, // tag type (video)
            0x00, 0x00, 0x00, 0x00, // data size (0 bytes for header)
            0x00, 0x00, 0x00, 0x00, // timestamp (0 ms)
            0x00, // stream ID (always 0)
        ];

        let mut decoder = FlvDecoder::new();
        let mut buffer = BytesMut::from(data.as_slice());

        let result = decoder.decode(&mut buffer).unwrap();

        assert!(result.is_some());
        if let Some(FlvData::Header(header)) = result {
            assert_eq!(header.version, 1);
        } else {
            panic!("Expected FLV header");
        }
    }

    #[test]
    fn test_flv_decoder_stream() {
        let data = vec![
            0x46, 0x4C, 0x56, // "FLV" signature
            0x01, // version
            0x05, // flags (audio and video)
            0x00, 0x00, 0x00, 0x09, // data offset (9 bytes)
            // FLV tag header (11 bytes)
            0x09, // tag type (video)
            0x00, 0x00, 0x00, 0x00, // data size (0 bytes for header)
            0x00, 0x00, 0x00, 0x00, // timestamp (0 ms)
            0x00, // stream ID (always 0)
        ];

        let cursor = Cursor::new(data);
        let mut decoder_stream = FlvDecoderStream::new(cursor);

        let result = futures::executor::block_on(decoder_stream.next());

        assert!(result.is_some());
        if let Some(Ok(FlvData::Header(header))) = result {
            assert_eq!(header.version, 1);
        } else {
            panic!("Expected FLV header");
        }
    }

    #[tokio::test]
    async fn test_flv_parser_invalid() {
        let path = Path::new("invalid.flv");
        let parser = FlvParser::create_decoder_stream(&path).await;
        assert!(parser.is_err());
    }

    #[tokio::test]
    async fn test_read_file_async() -> Result<(), Box<dyn std::error::Error>> {
        let path = Path::new("D:/test/999/16_02_26-福州~ 主播恋爱脑！！！.flv");

        // Skip the test if the file doesn't exist
        if !path.exists() {
            println!("Test file not found, skipping test");
            return Ok(());
        }

        // Get file size before parsing
        let file_size = std::fs::metadata(path)?.len();
        let file_size_mb = file_size as f64 / (1024.0 * 1024.0);

        let start = Instant::now(); // Start timer

        // Create the decoder stream and count the tags
        let mut stream = FlvParser::create_decoder_stream(&path).await?;
        let mut tags_count = 0;
        let mut has_header = false;

        // Add these variables to track tag types
        let mut video_tags = 0;
        let mut audio_tags = 0;
        let mut metadata_tags = 0;

        // Process each item in the stream
        while let Some(result) = stream.next().await {
            match result {
                Ok(FlvData::Header(_)) => {
                    has_header = true;
                }
                Ok(FlvData::Tag(tag)) => {
                    tags_count += 1;
                    if tag.is_video_tag() {
                        video_tags += 1;
                    } else if tag.is_audio_tag() {
                        audio_tags += 1; // Audio tag
                    } else if tag.is_script_tag() {
                        metadata_tags += 1; // Script/metadata tag
                    }
                }
                Ok(FlvData::EndOfSequence(_)) => {
                    println!("End of sequence reached");
                    tags_count += 1; // Count the end of sequence as a tag
                }
                Err(e) => {
                    println!("Error processing tag: {:?}", e);
                    // break;
                }
            }
        }

        let duration = start.elapsed(); // Stop timer

        // Calculate read speed
        let seconds = duration.as_secs() as f64 + duration.subsec_nanos() as f64 * 1e-9;
        let speed_mbps = file_size_mb / seconds;

        println!("Parsed FLV file asynchronously in {:?}", duration);
        println!("File size: {:.2} MB", file_size_mb);
        println!("Read speed: {:.2} MB/s", speed_mbps);

        println!("Successfully parsed FLV file with {} tags", tags_count);
        println!(
            "Audio tags: {}, Video tags: {}, Metadata tags: {}",
            audio_tags, video_tags, metadata_tags
        );
        assert!(has_header, "Expected to parse an FLV header");
        assert!(tags_count > 0, "Expected to parse at least one tag");

        Ok(())
    }

    #[tokio::test]
    async fn test_compare_parsers() -> Result<(), Box<dyn std::error::Error>> {
        let path = Path::new("D:/test/999/16_02_26-福州~ 主播恋爱脑！！！.flv");

        // Skip the test if the file doesn't exist
        if !path.exists() {
            println!("Test file not found, skipping test");
            return Ok(());
        }

        // Parse using the synchronous parser
        let sync_start = Instant::now();
        let sync_tags_count = crate::parser::FlvParser::parse_file(path)?;
        let sync_duration = sync_start.elapsed();

        // Parse using the async parser and collect detailed tag information
        let async_start = Instant::now();
        let mut stream = FlvParser::create_decoder_stream(&path).await?;

        // Track all tags with their timestamps - we'll use these to compare
        let mut async_tag_counts = HashMap::new();
        let mut async_tag_timestamps = Vec::new();
        let mut async_tags_count = 0;

        // Process each item in the stream
        while let Some(result) = stream.next().await {
            match result {
                Ok(FlvData::Tag(tag)) => {
                    async_tags_count += 1;

                    // Categorize the tag by type
                    let tag_type = if tag.is_video_tag() {
                        "video"
                    } else if tag.is_audio_tag() {
                        "audio"
                    } else if tag.is_script_tag() {
                        "metadata"
                    } else {
                        "unknown"
                    };

                    // Count by type and timestamp
                    *async_tag_counts.entry(tag_type).or_insert(0) += 1;

                    // Store timestamp for pattern analysis
                    if async_tags_count % 1000 == 0 {
                        // Sample every 1000th tag to avoid excessive memory use
                        async_tag_timestamps.push((tag.timestamp_ms, tag_type.to_string()));
                    }
                }
                Ok(FlvData::Header(_)) => {}
                Ok(FlvData::EndOfSequence(_)) => {}
                Err(e) => {
                    println!("Error processing tag: {:?}", e);
                    // Don't break - continue processing
                }
            }
        }
        let async_duration = async_start.elapsed();

        // Now run a separate sync parser to collect timestamps
        let mut sync_tag_timestamps = Vec::new();
        let file = std::fs::File::open(path)?;
        let mut reader = std::io::BufReader::new(file);
        let mut buffer = BytesMut::with_capacity(9);

        // Read and skip header
        buffer.resize(9, 0);
        std::io::Read::read_exact(&mut reader, &mut buffer)?;

        // Create buffer for reading tags
        let mut tag_buffer = BytesMut::with_capacity(4 * 1024);
        let mut sync_tag_counter = 0;

        loop {
            // Skip previous tag size
            tag_buffer.resize(4, 0);
            if let Err(e) = std::io::Read::read_exact(&mut reader, &mut tag_buffer) {
                if e.kind() == std::io::ErrorKind::UnexpectedEof {
                    break;
                }
                return Err(e.into());
            }

            // Read tag header
            tag_buffer.resize(11, 0);
            if let Err(e) = std::io::Read::read_exact(&mut reader, &mut tag_buffer) {
                if e.kind() == std::io::ErrorKind::UnexpectedEof {
                    break;
                }
                return Err(e.into());
            }

            // Get tag type and size
            let tag_type = tag_buffer[0];
            let data_size = ((tag_buffer[1] as u32) << 16)
                | ((tag_buffer[2] as u32) << 8)
                | (tag_buffer[3] as u32);

            // Get timestamp
            let timestamp = ((tag_buffer[7] as u32) << 24)
                | ((tag_buffer[4] as u32) << 16)
                | ((tag_buffer[5] as u32) << 8)
                | (tag_buffer[6] as u32);

            // Record timestamp for sampled tags
            sync_tag_counter += 1;
            if sync_tag_counter % 1000 == 0 {
                let tag_type_str = match tag_type {
                    8 => "audio",
                    9 => "video",
                    18 => "metadata",
                    _ => "unknown",
                };
                sync_tag_timestamps.push((timestamp, tag_type_str.to_string()));
            }

            // Skip the tag data
            let mut data_buffer = vec![0; data_size as usize];
            if let Err(e) = std::io::Read::read_exact(&mut reader, &mut data_buffer) {
                if e.kind() == std::io::ErrorKind::UnexpectedEof {
                    break;
                }
                return Err(e.into());
            }
        }

        // Print comparison results
        println!("\n=== PARSER COMPARISON RESULTS ===");
        println!(
            "Sync parser: {} tags in {:?}",
            sync_tags_count, sync_duration
        );
        println!(
            "Async parser: {} tags in {:?}",
            async_tags_count, async_duration
        );
        println!(
            "Difference: {} tags",
            sync_tags_count as i64 - async_tags_count as i64
        );
        println!("\nAsync parser tag types:");
        for (tag_type, count) in &async_tag_counts {
            println!("  {}: {}", tag_type, count);
        }

        // Find timestamp discrepancies
        println!("\nTimestamp comparison (sample of every 1000th tag):");
        let min_samples = std::cmp::min(sync_tag_timestamps.len(), async_tag_timestamps.len());
        let mut first_discrepancy = None;

        for i in 0..min_samples {
            if sync_tag_timestamps[i] != async_tag_timestamps[i] {
                first_discrepancy = Some(i);
                break;
            }
        }

        if let Some(idx) = first_discrepancy {
            println!("First timestamp discrepancy at tag #{}:", idx * 1000);
            println!("  Sync: {:?}", sync_tag_timestamps[idx]);
            println!("  Async: {:?}", async_tag_timestamps[idx]);

            // Print surrounding context
            let start = if idx > 2 { idx - 2 } else { 0 };
            let end = if idx + 3 < min_samples {
                idx + 3
            } else {
                min_samples
            };

            println!("\nContext around discrepancy:");
            for i in start..end {
                println!(
                    "  Tag #{}: Sync {:?}, Async {:?}",
                    i * 1000,
                    sync_tag_timestamps[i],
                    async_tag_timestamps[i]
                );
            }
        } else {
            println!("No timestamp discrepancies found in sampled tags");
        }

        Ok(())
    }
}
