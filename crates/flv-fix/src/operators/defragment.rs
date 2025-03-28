//! # Defragment Operator
//!
//! The Defragment Operator is responsible for detecting and handling fragmented FLV streams.
//!
//! ## Purpose
//!
//! When recording live streams, the connection may be interrupted or the stream might start
//! mid-transmission. This can result in incomplete or fragmented FLV data that needs special
//! handling to ensure the output file is valid and playable.
//!
//! ## How it Works
//!
//! The operator uses a buffering strategy:
//!
//! 1. When an FLV header is detected, it starts "gathering" mode
//! 2. It buffers tags until a minimum threshold is reached
//! 3. If enough tags are gathered, the stream is considered valid and all buffered tags are emitted
//! 4. If another header is detected before gathering enough tags, the existing buffer is discarded
//!    (considered a fragmented/invalid segment)
//!
//! This approach helps filter out small, broken segments while preserving complete, valid ones.
//!
//! ## Configuration
//!
//! The operator requires a minimum number of tags (`MIN_TAGS_NUM`) to consider a segment valid.
//! This value can be adjusted based on stream characteristics.
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
use kanal::{AsyncReceiver, AsyncSender};
use log::{debug, warn};
use std::sync::Arc;

/// An operator that buffers and validates FLV stream fragments to ensure continuity and validity.
///
/// The DefragmentOperator helps manage fragmented streams by:
/// - Buffering a minimum number of tags after each header
/// - Only emitting complete segments with enough tags
/// - Discarding small fragments that might be incomplete
/// - Handling errors appropriately throughout the process
pub struct DefragmentOperator {
    context: Arc<StreamerContext>,
}

impl DefragmentOperator {
    /// Creates a new DefragmentOperator with the given context.
    ///
    /// # Arguments
    ///
    /// * `context` - The shared StreamerContext containing configuration and state
    pub fn new(context: Arc<StreamerContext>) -> Self {
        Self { context }
    }

    /// Processes the input stream, buffering and validating fragments before passing them to output.
    ///
    /// # Arguments
    ///
    /// * `input` - The receiver channel for incoming FLV data
    /// * `output` - The sender channel for validated output FLV data
    ///
    /// # Behavior
    ///
    /// - When a header is received, starts buffering tags
    /// - If enough tags are received (`MIN_TAGS_NUM`), emits the entire buffer
    /// - If another header arrives before gathering enough tags, discards the buffer
    /// - Propagates errors while clearing any in-progress buffering
    pub async fn process(
        &self,
        mut input: AsyncReceiver<Result<FlvData, FlvError>>,
        output: AsyncSender<Result<FlvData, FlvError>>,
    ) {
        const MIN_TAGS_NUM: usize = 10;
        let mut is_gathering = false;
        let mut buffer = Vec::with_capacity(MIN_TAGS_NUM);

        while let Ok(item) = input.recv().await {
            match item {
                Ok(data) => {
                    if matches!(data, FlvData::Header(_)) {
                        if !buffer.is_empty() {
                            warn!(
                                "{} Discarded {} items, total size: {}",
                                self.context.name,
                                buffer.len(),
                                buffer.iter().map(|d: &FlvData| d.size()).sum::<usize>(),
                            );
                            buffer.clear();
                        }
                        is_gathering = true;
                        debug!("{} Start gathering...", self.context.name);
                    }

                    if is_gathering {
                        buffer.push(data);

                        if buffer.len() >= MIN_TAGS_NUM {
                            debug!(
                                "{} Gathered {} items, total size: {}",
                                self.context.name,
                                buffer.len(),
                                buffer.iter().map(|d| d.size()).sum::<usize>(),
                            );

                            // Emit all buffered items
                            for tag in buffer.drain(..) {
                                if output.send(Ok(tag)).await.is_err() {
                                    return;
                                }
                            }

                            is_gathering = false;

                            debug!(
                                "{} Not a fragmented sequence, stopped checking...",
                                self.context.name,
                            );
                            // Reset buffer for next sequence
                            buffer.clear();
                        }
                    } else {
                        // Not gathering, emit immediately
                        if output.send(Ok(data)).await.is_err() {
                            return;
                        }
                    }
                }
                Err(e) => {
                    // Clear buffer and propagate error
                    buffer.clear();
                    is_gathering = false;
                    if output.send(Err(e)).await.is_err() {
                        return;
                    }
                }
            }
        }

        // Handle any remaining data at end of stream
        if !buffer.is_empty() {
            if buffer.len() >= MIN_TAGS_NUM {
                // If we have enough items, consider it valid
                for tag in buffer {
                    if output.send(Ok(tag)).await.is_err() {
                        return;
                    }
                }
            } else {
                // Not enough data, discard as fragmented
                warn!(
                    "{} End of stream with only {} items in buffer, discarding",
                    self.context.name,
                    buffer.len(),
                );
            }
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use bytes::Bytes;
    use flv::{
        data::FlvData,
        header::FlvHeader,
        tag::{FlvTag, FlvTagType},
    };
    use kanal;
    use std::time::Duration;

    // Helper function to create a test context
    fn create_test_context() -> Arc<StreamerContext> {
        Arc::new(StreamerContext::default())
    }

    // Helper function to create a FlvHeader for testing
    fn create_test_header() -> FlvData {
        FlvData::Header(FlvHeader::new(true, true))
    }

    // Helper function to create a FlvTag for testing
    fn create_test_tag(tag_type: FlvTagType, timestamp: u32) -> FlvData {
        let data = vec![0u8; 10]; // Sample tag data
        FlvData::Tag(FlvTag {
            timestamp_ms: timestamp,
            stream_id: 0,
            tag_type,
            data: Bytes::from(data),
        })
    }

    #[tokio::test]
    async fn test_normal_flow_with_enough_tags() {
        let context = create_test_context();
        let operator = DefragmentOperator::new(context);

        let (input_tx, input_rx) = kanal::bounded_async(32);
        let (output_tx, mut output_rx) = kanal::bounded_async(32);

        // Start the process in a separate task
        tokio::spawn(async move {
            operator.process(input_rx, output_tx).await;
        });

        // Send a header followed by enough tags to trigger emission
        input_tx.send(Ok(create_test_header())).await.unwrap();
        for i in 0..11 {
            input_tx
                .send(Ok(create_test_tag(FlvTagType::Video, i)))
                .await
                .unwrap();
        }

        // Close the input
        drop(input_tx);

        // Should receive all 12 items (header + 11 tags)
        let mut count = 0;
        while let Ok(data) = output_rx.recv().await {
            count += 1;
        }

        assert_eq!(count, 12);
    }

    #[tokio::test]
    async fn test_header_reset() {
        let context = create_test_context();
        let operator = DefragmentOperator::new(context);

        let (input_tx, input_rx) = kanal::bounded_async(32);
        let (output_tx, mut output_rx) = kanal::bounded_async(32);

        // Start the process in a separate task
        tokio::spawn(async move {
            operator.process(input_rx, output_tx).await;
        });

        // Send a header followed by some tags (but not enough to emit)
        input_tx.send(Ok(create_test_header())).await.unwrap();
        for i in 0..5 {
            input_tx
                .send(Ok(create_test_tag(FlvTagType::Video, i)))
                .await
                .unwrap();
        }

        // Send another header (should discard previous tags)
        input_tx.send(Ok(create_test_header())).await.unwrap();
        for i in 0..11 {
            input_tx
                .send(Ok(create_test_tag(FlvTagType::Video, i)))
                .await
                .unwrap();
        }

        // Send regular tag that should be emitted immediately
        input_tx
            .send(Ok(create_test_tag(FlvTagType::Video, 100)))
            .await
            .unwrap();

        // Close the input
        drop(input_tx);

        // Should receive 13 items (header + 11 tags from second batch + 1 regular tag)
        let mut count = 0;
        while let Ok(_) = output_rx.recv().await {
            count += 1;
        }

        assert_eq!(count, 13);
    }

    #[tokio::test]
    async fn test_error_propagation() {
        let context = create_test_context();
        let operator = DefragmentOperator::new(context);

        let (input_tx, input_rx) = kanal::bounded_async(32);
        let (output_tx, mut output_rx) = kanal::bounded_async(32);

        // Start the process in a separate task
        tokio::spawn(async move {
            operator.process(input_rx, output_tx).await;
        });

        // Send some valid data
        input_tx.send(Ok(create_test_header())).await.unwrap();
        for i in 0..3 {
            input_tx
                .send(Ok(create_test_tag(FlvTagType::Video, i)))
                .await
                .unwrap();
        }

        // Send an error
        input_tx
            .send(Err(FlvError::IoError(std::io::Error::new(
                std::io::ErrorKind::Other,
                "Test error",
            ))))
            .await
            .unwrap();

        // Send more valid data
        for i in 4..7 {
            input_tx
                .send(Ok(create_test_tag(FlvTagType::Video, i)))
                .await
                .unwrap();
        }

        // Close the input
        drop(input_tx);

        // Collect the results
        let mut results = Vec::new();
        while let Ok(result) = output_rx.recv().await {
            results.push(result);
        }

        // Should have at least one error
        let error_count = results.iter().filter(|r| r.is_err()).count();
        assert_eq!(error_count, 1);
    }

    #[tokio::test]
    async fn test_end_of_stream_with_enough_tags() {
        let context = create_test_context();
        let operator = DefragmentOperator::new(context);

        let (input_tx, input_rx) = kanal::bounded_async(32);
        let (output_tx, mut output_rx) = kanal::bounded_async(32);

        // Start the process in a separate task
        tokio::spawn(async move {
            operator.process(input_rx, output_tx).await;
        });

        // Send a header followed by exactly MIN_TAGS_NUM tags
        input_tx.send(Ok(create_test_header())).await.unwrap();
        for i in 0..10 {
            input_tx
                .send(Ok(create_test_tag(FlvTagType::Video, i)))
                .await
                .unwrap();
        }

        // Close the input
        drop(input_tx);

        // Should receive all 11 items (header + 10 tags)
        let mut count = 0;
        while let Ok(_) = output_rx.recv().await {
            count += 1;
        }

        assert_eq!(count, 11);
    }

    #[tokio::test]
    async fn test_end_of_stream_with_insufficient_tags() {
        let context = create_test_context();
        let operator = DefragmentOperator::new(context);

        let (input_tx, input_rx) = kanal::bounded_async(32);
        let (output_tx, mut output_rx) = kanal::bounded_async(32);

        // Start the process in a separate task
        tokio::spawn(async move {
            operator.process(input_rx, output_tx).await;
        });

        // Send a header followed by fewer than MIN_TAGS_NUM tags
        input_tx.send(Ok(create_test_header())).await.unwrap();
        for i in 0..5 {
            input_tx
                .send(Ok(create_test_tag(FlvTagType::Video, i)))
                .await
                .unwrap();
        }

        // Close the input
        drop(input_tx);

        // All items should be discarded
        let mut count = 0;
        let timeout = tokio::time::timeout(Duration::from_millis(100), async {
            while let Ok(_) = output_rx.recv().await {
                count += 1;
            }
        })
        .await;

        assert!(timeout.is_ok());
        assert_eq!(count, 0);
    }
}
