use crate::context::StreamerContext;
use crate::error::FlvError;
use flv::data::FlvData;
use flv::header::FlvHeader;
use log::warn;
use std::sync::Arc;
use tokio::sync::mpsc::{Receiver, Sender};

pub struct HeaderCheckOperator {
    context: Arc<StreamerContext>,
}

impl HeaderCheckOperator {
    pub fn new(context: Arc<StreamerContext>) -> Self {
        Self { context }
    }

    /// Process the input stream and ensure it starts with a valid FLV header.
    /// If no header is present at the beginning, a default one will be inserted.
    pub async fn process(
        &self,
        mut input: Receiver<Result<FlvData, FlvError>>,
        output: Sender<Result<FlvData, FlvError>>,
    ) {
        let mut first_item = true;

        while let Some(item) = input.recv().await {
            match item {
                Ok(data) => {
                    if first_item {
                        first_item = false;
                        
                        // If the first item is not a header, insert a default one
                        if !matches!(data, FlvData::Header(_)) {
                            warn!(
                                "{} FLV header is missing, inserted a default header",
                                self.context.name
                            );
                            // Send a default header
                            let default_header = FlvHeader::new(true, true);
                            if output.send(Ok(FlvData::Header(default_header))).await.is_err() {
                                return;
                            }
                        }
                    }
                    
                    // Forward the data
                    if output.send(Ok(data)).await.is_err() {
                        return;
                    }
                }
                Err(e) => {
                    // Error handling - just forward the error
                    first_item = false;
                    if output.send(Err(e)).await.is_err() {
                        return;
                    }
                }
            }
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use bytes::Bytes;
    use flv::tag::{FlvTag, FlvTagType};
    use tokio::sync::mpsc;

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
    async fn test_with_header_present() {
        let context = create_test_context();
        let operator = HeaderCheckOperator::new(context);

        let (input_tx, input_rx) = mpsc::channel(32);
        let (output_tx, mut output_rx) = mpsc::channel(32);

        // Start the process in a separate task
        tokio::spawn(async move {
            operator.process(input_rx, output_tx).await;
        });

        // Send a header followed by some tags
        input_tx.send(Ok(create_test_header())).await.unwrap();
        for i in 0..5 {
            input_tx
                .send(Ok(create_test_tag(FlvTagType::Video, i)))
                .await
                .unwrap();
        }

        // Close the input
        drop(input_tx);

        // Should receive all 6 items (header + 5 tags)
        let mut received_items = Vec::new();
        while let Some(item) = output_rx.recv().await {
            received_items.push(item.unwrap());
        }

        assert_eq!(received_items.len(), 6);
        
        // First item should be a header
        assert!(matches!(received_items[0], FlvData::Header(_)));
    }

    #[tokio::test]
    async fn test_without_header() {
        let context = create_test_context();
        let operator = HeaderCheckOperator::new(context);

        let (input_tx, input_rx) = mpsc::channel(32);
        let (output_tx, mut output_rx) = mpsc::channel(32);

        // Start the process in a separate task
        tokio::spawn(async move {
            operator.process(input_rx, output_tx).await;
        });

        // Send tags without a header
        for i in 0..5 {
            input_tx
                .send(Ok(create_test_tag(FlvTagType::Video, i)))
                .await
                .unwrap();
        }

        // Close the input
        drop(input_tx);

        // Should receive 6 items (default header + 5 tags)
        let mut received_items = Vec::new();
        while let Some(item) = output_rx.recv().await {
            received_items.push(item.unwrap());
        }

        assert_eq!(received_items.len(), 6);
        
        // First item should be a header
        assert!(matches!(received_items[0], FlvData::Header(_)));
    }

    #[tokio::test]
    async fn test_with_error() {
        let context = create_test_context();
        let operator = HeaderCheckOperator::new(context);

        let (input_tx, input_rx) = mpsc::channel(32);
        let (output_tx, mut output_rx) = mpsc::channel(32);

        // Start the process in a separate task
        tokio::spawn(async move {
            operator.process(input_rx, output_tx).await;
        });

        // Send an error as the first item
        input_tx
            .send(Err(FlvError::IoError(std::io::Error::new(
                std::io::ErrorKind::Other,
                "Test error",
            ))))
            .await
            .unwrap();

        // Send some valid data after the error
        for i in 0..3 {
            input_tx
                .send(Ok(create_test_tag(FlvTagType::Video, i)))
                .await
                .unwrap();
        }

        // Close the input
        drop(input_tx);

        // Should receive 4 items (1 error + 3 tags)
        let mut received_items = Vec::new();
        while let Some(item) = output_rx.recv().await {
            received_items.push(item);
        }

        assert_eq!(received_items.len(), 4);
        
        // First item should be an error
        assert!(received_items[0].is_err());
        
        // No header should be inserted after an error
        assert!(matches!(received_items[1], Ok(FlvData::Tag(_))));
    }
}
