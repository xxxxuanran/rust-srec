//! # TimeConsistencyOperator
//!
//! The `TimeConsistencyOperator` ensures timestamp continuity in FLV streams
//! that have been split due to parameter changes.
//!
//! ## Purpose
//!
//! When stream splits occur (due to resolution, codec, or other parameter changes),
//! reinjected initialization data can cause timestamp discontinuities. This operator:
//!
//! 1. Detects split points in the stream (header reinjection)
//! 2. Calculates appropriate timestamp offsets for each segment
//! 3. Adjusts timestamps to maintain a continuous timeline
//! 4. Preserves relative timing within each segment
//!
//! ## Operation
//!
//! The operator:
//! - Tracks FLV headers to identify stream split points
//! - Records the last timestamp before a split and first timestamp after
//! - Calculates appropriate offsets to maintain timeline continuity
//! - Applies these offsets to all subsequent tags until the next split
//!
//! ## Configuration
//!
//! The operator supports two timeline continuity modes:
//! - `Continuous`: Maintains an ever-increasing timeline across all segments
//! - `Reset`: Resets the timeline to zero at each split point
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
use crate::operators::{FlvOperator, FlvProcessor};
use flv::data::FlvData;
use flv::error::FlvError;
use flv::tag::FlvUtil;
use kanal::AsyncReceiver as Receiver;
use kanal::AsyncSender as Sender;
use std::cmp::max;
use std::sync::Arc;
use tracing::{debug, trace};

/// Defines how timestamps should be handled across stream splits
#[derive(Debug, Clone, Copy, PartialEq)]
pub enum ContinuityMode {
    /// Maintain a continuous timeline across all segments
    Continuous,

    /// Reset timeline to zero after each split
    Reset,
}

impl Default for ContinuityMode {
    fn default() -> Self {
        Self::Continuous
    }
}

/// State tracking for timestamp correction
struct TimelineState {
    /// Whether we've seen a header and are starting a new segment
    new_segment: bool,

    /// Tracking of segments for debugging
    segment_count: u32,

    /// Last timestamp seen in the previous segment
    last_timestamp: Option<u32>,

    /// First timestamp seen in the current segment
    first_timestamp_in_segment: Option<u32>,

    /// Current timestamp offset to apply
    timestamp_offset: i64,

    /// Whether we need to calculate a new offset
    needs_offset_calculation: bool,
}

impl TimelineState {
    fn new() -> Self {
        Self {
            new_segment: true,
            segment_count: 0,
            last_timestamp: None,
            first_timestamp_in_segment: None,
            timestamp_offset: 0,
            needs_offset_calculation: false,
        }
    }

    fn reset(&mut self) {
        self.segment_count += 1;
        self.new_segment = true;
        self.first_timestamp_in_segment = None;
        self.needs_offset_calculation = true;
        self.timestamp_offset = 0
    }
}

/// Operator that corrects timestamp discontinuities after stream splits
pub struct TimeConsistencyOperator {
    context: Arc<StreamerContext>,
    continuity_mode: ContinuityMode,
    state: TimelineState,
}

impl TimeConsistencyOperator {
    /// Create a new TimeConsistencyOperator
    pub fn new(context: Arc<StreamerContext>, continuity_mode: ContinuityMode) -> Self {
        Self {
            context,
            continuity_mode,
            state: TimelineState::new(),
        }
    }

    /// Calculate timestamp offset based on continuity mode and current state
    fn calculate_timestamp_offset(&mut self) {
        if let (Some(last), Some(first)) = (
            self.state.last_timestamp,
            self.state.first_timestamp_in_segment,
        ) {
            match self.continuity_mode {
                ContinuityMode::Continuous => {
                    // Make current segment continue from where the previous one ended
                    self.state.timestamp_offset = last as i64 - first as i64;
                    debug!(
                        "{} Maintaining continuous timeline: offset = {}ms",
                        self.context.name, self.state.timestamp_offset
                    );
                }
                ContinuityMode::Reset => {
                    // Reset timeline - this means applying a negative offset to bring timestamps to zero
                    self.state.timestamp_offset = -(first as i64);
                    debug!(
                        "{} Resetting timeline to zero: offset = {}ms",
                        self.context.name, self.state.timestamp_offset
                    );
                }
            }
        }
        self.state.needs_offset_calculation = false;
    }
}

impl FlvProcessor for TimeConsistencyOperator {
    fn process(
        &mut self,
        input: FlvData,
        output: &mut dyn FnMut(FlvData) -> Result<(), FlvError>,
    ) -> Result<(), FlvError> {
        match input {
            FlvData::Header(_) => {
                // Headers indicate stream splits (except the first one)
                if self.state.segment_count > 0 {
                    debug!(
                        "{} Detected stream split, preparing timestamp correction",
                        self.context.name
                    );
                }
                self.state.reset();

                // Forward the header unmodified
                output(input)
            }
            FlvData::Tag(mut tag) => {
                let original_timestamp = tag.timestamp_ms;

                // For normal media tags, handle timestamp adjustment
                if self.state.new_segment {
                    // For sequence headers, always set timestamp to 0
                    if tag.is_video_sequence_header() || tag.is_audio_sequence_header() {
                        // Save original timestamp for debugging
                        let original = tag.timestamp_ms;
                        if original != 0 {
                            debug!(
                                "{} Reset sequence header timestamp from {}ms to 0ms",
                                self.context.name, original
                            );
                            // Set timestamp to 0
                            tag.timestamp_ms = 0;
                        }

                        return output(FlvData::Tag(tag));
                    } else if tag.is_script_tag() {
                        // apply delta to script data tags
                        if self.state.timestamp_offset != 0 {
                            tag.timestamp_ms = 0;

                            debug!(
                                "{} Adjusted script data timestamp: {}ms -> {}ms",
                                self.context.name, original_timestamp, 0
                            );
                        }
                        return output(FlvData::Tag(tag));
                    }

                    if self.state.first_timestamp_in_segment.is_none() {
                        // Record the first timestamp in this segment
                        self.state.first_timestamp_in_segment = Some(tag.timestamp_ms);
                        debug!(
                            "{} First timestamp in segment {}: {}ms",
                            self.context.name, self.state.segment_count, tag.timestamp_ms
                        );

                        if self.state.segment_count > 1 && self.state.needs_offset_calculation {
                            self.calculate_timestamp_offset();
                        } else if self.state.segment_count == 1
                            && self.continuity_mode == ContinuityMode::Reset
                        {
                            // use the first timestamp as the delta
                            self.state.timestamp_offset =
                                -(self.state.first_timestamp_in_segment.unwrap() as i64);
                        }
                    }
                    self.state.new_segment = false;
                }

                // Apply timestamp correction if needed
                if self.state.timestamp_offset != 0 {
                    // Calculate the corrected timestamp, ensure it doesn't go negative
                    let corrected =
                        max(0, tag.timestamp_ms as i64 + self.state.timestamp_offset) as u32;
                    tag.timestamp_ms = corrected;

                    trace!(
                        "{} Adjusted timestamp: {}ms -> {}ms",
                        self.context.name, original_timestamp, corrected
                    );
                }

                // Remember the last timestamp we've seen
                self.state.last_timestamp = Some(tag.timestamp_ms);

                // Forward the tag with possibly adjusted timestamp
                output(FlvData::Tag(tag))
            }
            // Forward other data types unmodified
            _ => output(input),
        }
    }

    fn finish(
        &mut self,
        _output: &mut dyn FnMut(FlvData) -> Result<(), FlvError>,
    ) -> Result<(), FlvError> {
        debug!("{} Time consistency operator completed", self.context.name);
        Ok(())
    }

    fn name(&self) -> &'static str {
        "TimeConsistencyOperator"
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use bytes::Bytes;
    use flv::{
        header::FlvHeader,
        tag::{FlvTag, FlvTagType},
    };
    use kanal;

    // Helper functions (similar to those in SplitOperator for consistency)
    fn create_test_context() -> Arc<StreamerContext> {
        Arc::new(StreamerContext::default())
    }

    fn create_header() -> FlvData {
        FlvData::Header(FlvHeader::new(true, true))
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

    fn create_audio_tag(timestamp: u32) -> FlvData {
        let data = vec![0xAF, 0x01, 0x21, 0x10, 0x04];
        FlvData::Tag(FlvTag {
            timestamp_ms: timestamp,
            stream_id: 0,
            tag_type: FlvTagType::Audio,
            data: Bytes::from(data),
        })
    }

    #[tokio::test]
    async fn test_continuous_mode() {
        let context = create_test_context();
        let mut operator = TimeConsistencyOperator::new(context, ContinuityMode::Continuous);

        let (input_tx, input_rx) = kanal::bounded_async(32);
        let (output_tx, output_rx) = kanal::bounded_async(32);

        tokio::spawn(async move {
            operator.process(input_rx, output_tx).await;
        });

        // First segment
        input_tx.send(Ok(create_header())).await.unwrap();

        // Send some tags with increasing timestamps
        for i in 0..5 {
            input_tx.send(Ok(create_video_tag(i * 100))).await.unwrap();
        }

        // Last timestamp in first segment: 400ms

        // Create a split (send another header)
        input_tx.send(Ok(create_header())).await.unwrap();

        // Second segment starts with timestamp 0 again
        for i in 0..5 {
            input_tx.send(Ok(create_video_tag(i * 100))).await.unwrap();
        }

        // Close input
        drop(input_tx);

        // Collect results and verify timestamps
        let mut results = Vec::new();
        while let Ok(result) = output_rx.recv().await {
            results.push(result.unwrap());
        }

        // In continuous mode, second segment timestamps should continue from 400ms
        // So instead of [0, 100, 200, 300, 400] we should see approximately [401, 501, 601, 701, 801]

        for (i, item) in results.iter().enumerate() {
            if let FlvData::Tag(tag) = item {
                println!("Tag {}: timestamp = {}ms", i, tag.timestamp_ms);
            } else if let FlvData::Header(_) = item {
                println!("Tag {}: FLV header", i);
            }
        }

        // Check specific timestamps in second segment
        // Headers at index 0 and 6
        // First segment tags at index 1-5
        // Second segment adjusted tags at index 7-11
        if let FlvData::Tag(tag) = &results[7] {
            assert!(
                tag.timestamp_ms >= 400,
                "First tag after split should have timestamp > 400ms"
            );
        }
    }

    #[tokio::test]
    async fn test_reset_mode() {
        let context = create_test_context();
        let mut operator = TimeConsistencyOperator::new(context, ContinuityMode::Reset);

        let (input_tx, input_rx) = kanal::bounded_async(32);
        let (output_tx, output_rx) = kanal::bounded_async(32);

        tokio::spawn(async move {
            operator.process(input_rx, output_tx).await;
        });

        // First segment
        input_tx.send(Ok(create_header())).await.unwrap();

        // Send some tags with increasing timestamps
        for i in 0..5 {
            input_tx
                .send(Ok(create_video_tag(i * 100 + 50)))
                .await
                .unwrap();
        }

        // Create a split (send another header)
        input_tx.send(Ok(create_header())).await.unwrap();

        // Second segment with timestamps starting at 80ms (non-zero to verify reset)
        for i in 0..5 {
            input_tx
                .send(Ok(create_video_tag(i * 100 + 80)))
                .await
                .unwrap();
        }

        // Close input
        drop(input_tx);

        // Collect results and verify timestamps
        let mut results = Vec::new();
        while let Ok(result) = output_rx.recv().await {
            results.push(result.unwrap());
        }

        // In reset mode, second segment timestamps should start near zero

        for (i, item) in results.iter().enumerate() {
            if let FlvData::Tag(tag) = item {
                println!("Tag {}: timestamp = {}ms", i, tag.timestamp_ms);
            } else if let FlvData::Header(_) = item {
                println!("Tag {}: FLV header", i);
            }
        }

        // First timestamp in second segment should be reset or near zero
        if let FlvData::Tag(tag) = &results[7] {
            assert!(
                tag.timestamp_ms < 10,
                "First tag after split should be near 0ms in reset mode"
            );
        }
    }

    #[tokio::test]
    async fn test_multiple_splits() {
        let context = create_test_context();
        let mut operator = TimeConsistencyOperator::new(context, ContinuityMode::Continuous);

        let (input_tx, input_rx) = kanal::bounded_async(32);
        let (output_tx, output_rx) = kanal::bounded_async(32);

        tokio::spawn(async move {
            operator.process(input_rx, output_tx).await;
        });

        // First segment
        input_tx.send(Ok(create_header())).await.unwrap();
        for i in 0..3 {
            input_tx.send(Ok(create_video_tag(i * 100))).await.unwrap();
        }
        // Last timestamp in first segment: 200ms

        // Second segment
        input_tx.send(Ok(create_header())).await.unwrap();
        for i in 0..3 {
            input_tx.send(Ok(create_video_tag(i * 100))).await.unwrap();
        }
        // Last timestamp in second segment: 200ms (before adjustment)

        // Third segment
        input_tx.send(Ok(create_header())).await.unwrap();
        for i in 0..3 {
            input_tx.send(Ok(create_video_tag(i * 100))).await.unwrap();
        }
        // Last timestamp in third segment: 200ms (before adjustment)

        // Close input
        drop(input_tx);

        // Collect results
        let mut results = Vec::new();
        while let Ok(result) = output_rx.recv().await {
            results.push(result.unwrap());
        }

        // Extract video tags by segment for comparison
        let mut segments: Vec<Vec<u32>> = Vec::new();
        let mut current_segment: Vec<u32> = Vec::new();

        for item in &results {
            match item {
                FlvData::Header(_) => {
                    if !current_segment.is_empty() {
                        segments.push(current_segment);
                        current_segment = Vec::new();
                    }
                }
                FlvData::Tag(tag) if tag.tag_type == FlvTagType::Video => {
                    current_segment.push(tag.timestamp_ms);
                }
                _ => {}
            }
        }

        if !current_segment.is_empty() {
            segments.push(current_segment);
        }

        // Display segments for analysis
        for (i, segment) in segments.iter().enumerate() {
            println!("Segment {}: {:?}", i, segment);
        }

        // Verify timestamp continuity
        assert_eq!(segments.len(), 3, "Should have 3 segments");

        // First segment should start near 0
        assert!(segments[0][0] < 10, "First segment should start near 0");

        // Second segment should start after the end of first segment
        assert!(
            segments[1][1] > segments[0][segments[0].len() - 1],
            "Second segment should continue after first segment"
        );

        // Third segment should start after the end of second segment
        assert!(
            segments[2][1] > segments[1][segments[1].len() - 1],
            "Third segment should continue after second segment"
        );

        // Check expected offsets (assuming 200ms per segment)
        assert_eq!(segments[0], vec![0, 100, 200], "First segment timestamps");
        assert!(
            segments[1][0] >= 200,
            "Second segment should start at or after 200ms"
        );
        assert!(
            segments[2][0] >= 400,
            "Third segment should start at or after 400ms"
        );
    }
}
