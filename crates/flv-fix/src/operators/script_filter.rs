//! # ScriptFilterOperator
//!
//! The `ScriptFilterOperator` removes duplicate script data (metadata) tags from an FLV stream.
//!
//! ## Purpose
//!
//! FLV streams may contain multiple script data tags (metadata), but many players
//! can only properly handle a single metadata tag. This operator ensures that:
//!
//! 1. The first script data tag is preserved (typically containing essential metadata)
//! 2. Subsequent script data tags are discarded
//!
//! This improves compatibility with various players and reduces unnecessary data.
//!
//! ## Operation
//!
//! The operator tracks whether it has already encountered a script tag. Once it has
//! seen the first script tag, it filters out any subsequent ones while passing through
//! all other tag types unmodified.
//!
//!
//! ## License
//!
//! MIT License
//!
//! ## Authors
//!
//! - hua0512
//!

use crate::context::StreamerContext;
use flv::data::FlvData;
use flv::error::FlvError;
use flv::tag::FlvTagType;
use std::sync::Arc;
use tracing::{debug, info};

use super::FlvProcessor;

/// Operator that filters out script data tags except for the first one
pub struct ScriptFilterOperator {
    context: Arc<StreamerContext>,
    seen_script_tag: bool,
    script_tag_count: u32,
}

impl ScriptFilterOperator {
    /// Create a new ScriptFilterOperator
    pub fn new(context: Arc<StreamerContext>) -> Self {
        Self {
            context,
            seen_script_tag: false,
            script_tag_count: 0,
        }
    }
}

impl FlvProcessor for ScriptFilterOperator {
    fn process(
        &mut self,
        input: FlvData,
        output: &mut dyn FnMut(FlvData) -> Result<(), FlvError>,
    ) -> Result<(), FlvError> {
        match input {
            FlvData::Header(_) => {
                debug!("{} Resetting script tag filter state", self.context.name);
                // Reset state on header
                self.seen_script_tag = false;
                self.script_tag_count = 0;
                // Forward the header
                output(input)
            } // Check if this is a script tag
            FlvData::Tag(tag) if tag.tag_type == FlvTagType::ScriptData => {
                self.script_tag_count += 1;
                if !self.seen_script_tag {
                    // First script tag, keep it and mark as seen
                    self.seen_script_tag = true;
                    debug!("{} Forwarding first script tag", self.context.name);
                    output(FlvData::Tag(tag))
                } else {
                    // Subsequent script tag, discard it
                    debug!(
                        "{} Discarding subsequent script tag #{}",
                        self.context.name, self.script_tag_count
                    );
                    // Skip sending this to output
                    Ok(())
                }
            }
            _ => output(input), // Forward other data types unmodified
        }
    }

    fn finish(
        &mut self,
        _output: &mut dyn FnMut(FlvData) -> Result<(), FlvError>,
    ) -> Result<(), FlvError> {
        if self.script_tag_count > 1 {
            info!(
                "{} Filtered out {} excess script tags",
                self.context.name,
                self.script_tag_count - 1
            );
        }
        debug!("{} Script filter operator completed", self.context.name);
        Ok(())
    }

    fn name(&self) -> &'static str {
        "ScriptFilterOperator"
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use bytes::Bytes;
    use flv::header::FlvHeader;
    use flv::tag::FlvTag;

    // Helper functions for testing
    fn create_test_context() -> Arc<StreamerContext> {
        Arc::new(StreamerContext::default())
    }

    fn create_header() -> FlvData {
        FlvData::Header(FlvHeader::new(true, true))
    }

    fn create_script_tag(timestamp: u32, data: Vec<u8>) -> FlvData {
        FlvData::Tag(FlvTag {
            timestamp_ms: timestamp,
            stream_id: 0,
            tag_type: FlvTagType::ScriptData,
            data: Bytes::from(data),
        })
    }

    fn create_video_tag(timestamp: u32) -> FlvData {
        let data = vec![0x17, 0x01, 0x00, 0x00, 0x00];
        FlvData::Tag(FlvTag {
            timestamp_ms: timestamp,
            stream_id: 0,
            tag_type: FlvTagType::Video,
            data: Bytes::from(data),
        })
    }

    #[tokio::test]
    async fn test_filter_script_tags() {
        let context = create_test_context();
        let mut operator = ScriptFilterOperator::new(context);

        let (input_tx, input_rx) = kanal::bounded_async(32);
        let (output_tx, output_rx) = kanal::bounded_async(32);

        tokio::spawn(async move {
            operator.process(input_rx, output_tx).await;
        });

        // Send header
        input_tx.send(Ok(create_header())).await.unwrap();

        // Send first script tag - should be kept
        input_tx
            .send(Ok(create_script_tag(0, vec![0x01, 0x02])))
            .await
            .unwrap();

        // Send some video tags
        input_tx.send(Ok(create_video_tag(10))).await.unwrap();
        input_tx.send(Ok(create_video_tag(20))).await.unwrap();

        // Send another script tag - should be discarded
        input_tx
            .send(Ok(create_script_tag(30, vec![0x03, 0x04])))
            .await
            .unwrap();

        // Send more video tags
        input_tx.send(Ok(create_video_tag(40))).await.unwrap();

        // Send a third script tag - should be discarded
        input_tx
            .send(Ok(create_script_tag(50, vec![0x05, 0x06])))
            .await
            .unwrap();

        // Close input
        drop(input_tx);

        // Collect results
        let mut results = Vec::new();
        while let Ok(result) = output_rx.recv().await {
            results.push(result.unwrap());
        }

        // Check we have the correct number of items (header + 1 script tag + 3 video tags = 5)
        assert_eq!(results.len(), 5, "Expected 5 items, got {}", results.len());

        // Verify the order and types of tags
        let mut tag_types = Vec::new();
        for item in &results {
            match item {
                FlvData::Header(_) => tag_types.push("Header"),
                FlvData::Tag(tag) => match tag.tag_type {
                    FlvTagType::ScriptData => tag_types.push("ScriptData"),
                    FlvTagType::Video => tag_types.push("Video"),
                    FlvTagType::Audio => tag_types.push("Audio"),
                    _ => tag_types.push("Unknown"),
                },
                _ => tag_types.push("Other"),
            }
        }

        // We expect: Header, ScriptData, Video, Video, Video
        assert_eq!(
            tag_types,
            vec!["Header", "ScriptData", "Video", "Video", "Video"]
        );
    }

    #[tokio::test]
    async fn test_multiple_headers_reset_filtering() {
        let context: Arc<StreamerContext> = create_test_context();
        let mut operator = ScriptFilterOperator::new(context);

        let (input_tx, input_rx) = kanal::bounded_async(32);
        let (output_tx, output_rx) = kanal::bounded_async(32);

        tokio::spawn(async move {
            operator.process(input_rx, output_tx).await;
        });

        // First segment
        input_tx.send(Ok(create_header())).await.unwrap();
        input_tx
            .send(Ok(create_script_tag(0, vec![0x01])))
            .await
            .unwrap();
        input_tx.send(Ok(create_video_tag(10))).await.unwrap();
        input_tx
            .send(Ok(create_script_tag(20, vec![0x02])))
            .await
            .unwrap(); // Should be discarded

        // Second segment (new header should reset filtering)
        input_tx.send(Ok(create_header())).await.unwrap();
        input_tx
            .send(Ok(create_script_tag(0, vec![0x03])))
            .await
            .unwrap(); // Should be kept
        input_tx.send(Ok(create_video_tag(10))).await.unwrap();
        input_tx
            .send(Ok(create_script_tag(20, vec![0x04])))
            .await
            .unwrap(); // Should be discarded

        // Close input
        drop(input_tx);

        // Collect results
        let mut results = Vec::new();
        while let Ok(result) = output_rx.recv().await {
            results.push(result.unwrap());
        }

        // Check we have the correct number of items (2 headers + 2 script tags + 2 video tags = 6)
        assert_eq!(results.len(), 6, "Expected 6 items, got {}", results.len());

        // Verify each segment has exactly one script tag
        let mut first_segment_script_count = 0;
        let mut second_segment_script_count = 0;
        // first element should be the header

        matches!(results[0], FlvData::Header(_));
        // The first segment is everything after the first header and before the second header
        let mut in_first_segment = true;

        // Iterate over the results starting from the second item
        for item in &results[1..] {
            match item {
                FlvData::Header(_) => {
                    in_first_segment = false; // Switch to second segment after seeing second header
                }
                FlvData::Tag(tag) => {
                    if tag.tag_type == FlvTagType::ScriptData {
                        if in_first_segment {
                            first_segment_script_count += 1;
                        } else {
                            second_segment_script_count += 1;
                        }
                    }
                }
                _ => {}
            }
        }

        assert_eq!(
            first_segment_script_count, 1,
            "First segment should have 1 script tag"
        );
        assert_eq!(
            second_segment_script_count, 1,
            "Second segment should have 1 script tag"
        );
    }
}
