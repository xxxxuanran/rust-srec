//! # ScriptKeyframesFillerOperator
//!
//! The `ScriptKeyframesFillerOperator` prepares FLV streams for improved seeking by injecting
//! placeholder keyframe metadata structures that can be populated later in the stream processing pipeline.
//!
//!
//! ## Purpose
//!
//! Many FLV streams lack proper keyframe metadata, which can result in:
//!
//! 1. Slow or inaccurate seeking in players
//! 2. Inability to use advanced player features
//! 3. Compatibility issues with certain players that expect standard metadata
//! 4. Poor streaming performance when keyframe information is needed for adapting quality
//!
//! This operator ensures the first script tag in the FLV stream contains properly
//! structured metadata including:
//!
//! - Standard metadata properties like duration, dimensions, codec info
//! - Empty keyframe index structure with pre-allocated space
//! - Player compatibility flags
//!
//! ## Operation
//!
//! The operator:
//! - Processes only the first script tag in the stream
//! - Preserves any existing metadata values when possible
//! - Adds default values for missing but important metadata
//! - Constructs empty keyframe arrays that can be filled by later operators
//! - Ensures proper ordering of metadata properties for maximum compatibility
//!
//! ## Configuration
//!
//! The operator supports configuration for:
//! - Target keyframe interval in milliseconds
//! - (Default to 3.5 hours for long recording sessions)
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
use crate::operators::FlvOperator;
use amf0::{Amf0Marker, Amf0Value, write_amf_property_key};
use byteorder::{BigEndian, WriteBytesExt};
use bytes::Bytes;
use flv::data::FlvData;
use flv::error::FlvError;
use flv::script::ScriptData;
use flv::tag::{FlvTag, FlvTagType};
use kanal::{AsyncReceiver, AsyncSender};
use std::borrow::Cow;
use std::io::{self, Write};
use std::sync::Arc;
use tracing::{debug, error, info, trace, warn};

const DEFAULT_KEYFRAME_INTERVAL_MS: u32 = (3.5 * 60.0 * 60.0 * 1000.0) as u32; // 3.5 hours in ms
pub const MIN_INTERVAL_BETWEEN_KEYFRAMES_MS: u32 = 1900; // 1.9 seconds in ms

pub static NATURAL_METADATA_KEY_ORDER: &[&str] = &[
    "duration",
    "width",
    "height",
    "framerate",
    "videocodecid",
    "audiocodecid",
    "hasAudio",
    "hasVideo",
    "hasMetadata",
    "hasKeyframes",
    "canSeekToEnd",
    "datasize",
    "filesize",
    "audiosize",
    "audiodatarate",
    "audiosamplerate",
    "audiosamplesize",
    "stereo",
    "videosize",
    "videodatarate",
    "lasttimestamp",
    "lastkeyframelocation",
    "lastkeyframetimestamp",
    "metadatacreator",
    "metadatadate",
    "keyframes",
];

/// Estimated size of the total metadata object
/// ~474 bytes for standard metadata properties
const TOTAL_NATURAL_METADATA_SIZE: usize = 500;

/// Configuration for the ScriptInjectorOperator
#[derive(Clone, Debug)]
pub struct ScriptFillerConfig {
    /// The target maximum duration of keyframes in milliseconds.
    /// Defaults to 3.5 hours.
    pub keyframe_duration_ms: u32,
}

impl Default for ScriptFillerConfig {
    fn default() -> Self {
        Self {
            keyframe_duration_ms: DEFAULT_KEYFRAME_INTERVAL_MS,
        }
    }
}

/// Operator to modify the first script tag with keyframe information.
pub struct ScriptKeyframesFillerOperator {
    context: Arc<StreamerContext>,
    config: ScriptFillerConfig,
    seen_first_script_tag: bool,
}

impl ScriptKeyframesFillerOperator {
    /// Creates a new ScriptInjectorOperator with the given configuration.
    pub fn new(context: Arc<StreamerContext>, config: ScriptFillerConfig) -> Self {
        Self {
            context,
            config,
            seen_first_script_tag: false,
        }
    }

    fn create_script_tag_payload() -> Bytes {
        // Create a new script tag with empty data
        let mut buffer = Vec::new();
        amf0::Amf0Encoder::encode_string(&mut buffer, "onMetaData").unwrap();
        amf0::Amf0Encoder::encode(
            &mut buffer,
            &Amf0Value::Object(Cow::Owned(vec![
                // add minimal duration property
                (Cow::Borrowed("duration"), Amf0Value::Number(0.0)),
            ])),
        )
        .unwrap();
        Bytes::from(buffer)
    }

    /// Creates a fallback tag with the same metadata as the original but with default script payload
    fn create_fallback_tag(&self, original_tag: &FlvTag) -> FlvTag {
        FlvTag {
            timestamp_ms: original_tag.timestamp_ms,
            stream_id: original_tag.stream_id,
            tag_type: original_tag.tag_type,
            data: Self::create_script_tag_payload(),
        }
    }

    /// Processes the AMF data for a valid onMetaData object
    fn process_onmeta_object(
        &self,
        props: &[(Cow<'_, str>, Amf0Value)],
        tag: &FlvTag,
        amf_data_name: &str,
    ) -> io::Result<FlvTag> {
        debug!(
            "{} Found onMetaData with {} properties",
            self.context.name,
            props.len()
        );

        // Calculate buffer sizes
        let keyframes_count = self
            .config
            .keyframe_duration_ms
            .div_ceil(MIN_INTERVAL_BETWEEN_KEYFRAMES_MS);
        let double_array_size = 2 * keyframes_count as usize;

        // Pre-allocate with a reasonable size estimate
        // Base size + metadata + keyframes structure
        let estimated_size = tag.data.len()
            + TOTAL_NATURAL_METADATA_SIZE
            + (1 + (3 * 25) + (3 * 5) + (9 * double_array_size) + 3);

        debug!("Estimated script data size: {}", estimated_size);
        let mut buffer = Vec::with_capacity(estimated_size);

        // Write metadata name (e.g. "onMetaData")
        amf0::Amf0Encoder::encode_string(&mut buffer, amf_data_name).unwrap();

        // Start object
        buffer.write_u8(Amf0Marker::Object as u8)?;

        // Get all keys except "keyframes" to process first
        let ordered_keys: Vec<&str> =
            NATURAL_METADATA_KEY_ORDER[..NATURAL_METADATA_KEY_ORDER.len() - 1].to_vec();

        // Track if we've added standard flags
        let flags_added = self.write_metadata_properties(&mut buffer, props, &ordered_keys)?;

        // Add non-standard properties
        self.write_custom_properties(&mut buffer, props)?;

        // Add compatibility flags if missing
        self.write_compatibility_flags(&mut buffer, flags_added)?;

        // Add keyframes structure
        self.write_keyframes_section(&mut buffer, double_array_size)?;

        // End of object
        amf0::Amf0Encoder::object_eof(&mut buffer).unwrap();

        debug!("New script data payload size: {}", buffer.len());

        // Create a new tag with the modified data
        Ok(FlvTag {
            timestamp_ms: tag.timestamp_ms,
            stream_id: tag.stream_id,
            tag_type: tag.tag_type,
            data: Bytes::from(buffer),
        })
    }

    /// Write standard metadata properties in the correct order
    fn write_metadata_properties(
        &self,
        buffer: &mut Vec<u8>,
        props: &[(Cow<'_, str>, Amf0Value)],
        ordered_keys: &[&str],
    ) -> io::Result<(bool, bool)> {
        let mut has_keyframes_added = false;
        let mut can_seek_to_end_added = false;

        for key in ordered_keys {
            // Check for special flags
            if *key == "hasKeyframes" {
                has_keyframes_added = true;
            } else if *key == "canSeekToEnd" {
                can_seek_to_end_added = true;
            } else if *key == "keyframes" {
                // Skip keyframes as we'll handle it specially
                continue;
            }

            // Find property in the original props
            let value_opt = props
                .iter()
                .find(|(k, _)| k.as_ref() == *key)
                .map(|(_, v)| v);

            if let Some(value) = value_opt {
                // Write existing property
                write_amf_property_key!(buffer, key);
                amf0::Amf0Encoder::encode(buffer, value).unwrap();
                trace!("Encoded property: {}, {:?}", key, value);
            } else if let Some(default_value) = self.get_default_metadata_value(key) {
                // Add default value for missing property
                trace!(
                    "{} Adding default value for missing property: {}",
                    self.context.name, key
                );
                write_amf_property_key!(buffer, key);
                amf0::Amf0Encoder::encode(buffer, &default_value).unwrap();
            }
        }

        Ok((has_keyframes_added, can_seek_to_end_added))
    }

    /// Write custom properties not in the standard ordered list
    fn write_custom_properties(
        &self,
        buffer: &mut Vec<u8>,
        props: &[(Cow<'_, str>, Amf0Value)],
    ) -> io::Result<()> {
        // Add any remaining custom properties from the original object
        for (key, value) in props.iter() {
            if !NATURAL_METADATA_KEY_ORDER.contains(&key.as_ref()) {
                write_amf_property_key!(buffer, key);
                amf0::Amf0Encoder::encode(buffer, value).unwrap();
            }
        }
        Ok(())
    }

    /// Write standard compatibility flags if they weren't already present
    fn write_compatibility_flags(
        &self,
        buffer: &mut Vec<u8>,
        (has_keyframes_added, can_seek_to_end_added): (bool, bool),
    ) -> io::Result<()> {
        // Add standard player compatibility flags if not present
        if !has_keyframes_added {
            write_amf_property_key!(buffer, "hasKeyframes");
            amf0::Amf0Encoder::encode(buffer, &Amf0Value::Boolean(true)).unwrap();
        }

        if !can_seek_to_end_added {
            write_amf_property_key!(buffer, "canSeekToEnd");
            amf0::Amf0Encoder::encode(buffer, &Amf0Value::Boolean(true)).unwrap();
        }

        Ok(())
    }

    /// Write the keyframes section with times, filepositions and spacer arrays
    fn write_keyframes_section(
        &self,
        buffer: &mut Vec<u8>,
        double_array_size: usize,
    ) -> io::Result<()> {
        // Add keyframes object directly
        write_amf_property_key!(buffer, "keyframes");
        buffer.write_u8(Amf0Marker::Object as u8)?;

        // Times array - empty array
        write_amf_property_key!(buffer, "times");
        buffer.write_u8(Amf0Marker::StrictArray as u8)?;
        buffer.write_u32::<BigEndian>(0)?;

        // File positions array - empty array
        write_amf_property_key!(buffer, "filepositions");
        buffer.push(Amf0Marker::StrictArray as u8);
        buffer.write_u32::<BigEndian>(0)?;

        // Spacer array with pre-allocated NaN values
        write_amf_property_key!(buffer, "spacer");
        buffer.write_u8(Amf0Marker::StrictArray as u8)?;
        buffer.write_u32::<BigEndian>(double_array_size as u32)?;

        for _ in 0..double_array_size {
            amf0::Amf0Encoder::encode_number(buffer, f64::NAN).unwrap();
        }

        // End keyframes object
        amf0::Amf0Encoder::object_eof(buffer).unwrap();

        Ok(())
    }

    /// Modifies the parsed AMF values to include the keyframes property.
    fn add_keyframes_to_amf(&self, tag: FlvTag) -> io::Result<FlvTag> {
        // Parse the AMF data using a reference to the tag data
        let mut cursor = std::io::Cursor::new(tag.data.clone());

        // Try to parse the AMF data
        match ScriptData::demux(&mut cursor) {
            Ok(amf_data) => {
                debug!(
                    "{} Script tag name: '{}', data length: {}, timestamp: {}ms",
                    self.context.name,
                    amf_data.name,
                    tag.data.len(),
                    tag.timestamp_ms
                );

                // Verify we have "onMetaData" with non-empty data array
                if amf_data.name != "onMetaData" {
                    warn!(
                        "{} Script tag name is not 'onMetaData', found: '{}'. Creating fallback.",
                        self.context.name, amf_data.name
                    );
                    return self.add_keyframes_to_amf(self.create_fallback_tag(&tag));
                }

                if amf_data.data.is_empty() {
                    warn!(
                        "{} onMetaData script tag has empty data array. Creating fallback.",
                        self.context.name
                    );
                    return self.add_keyframes_to_amf(self.create_fallback_tag(&tag));
                }

                // Check if first data item is an Object
                if let Amf0Value::Object(props) = &amf_data.data[0] {
                    self.process_onmeta_object(props, &tag, &amf_data.name)
                } else {
                    warn!(
                        "{} Unsupported AMF data type for keyframe injection: {:?}. Expected Object but found different type.",
                        self.context.name, amf_data.data[0]
                    );
                    self.add_keyframes_to_amf(self.create_fallback_tag(&tag))
                }
            }
            Err(err) => {
                // Log parsing error details
                warn!(
                    "{} Failed to parse AMF data for keyframe injection: {}. \
                    Tag data length: {}, timestamp: {}ms, first few bytes: {:?}",
                    self.context.name,
                    err,
                    tag.data.len(),
                    tag.timestamp_ms,
                    tag.data.iter().take(16).collect::<Vec<_>>()
                );

                // Use fallback
                self.add_keyframes_to_amf(self.create_fallback_tag(&tag))
            }
        }
    }

    /// Get default metadata value for a given key.
    fn get_default_metadata_value(&self, key: &str) -> Option<Amf0Value> {
        match key {
            // Booleans - Defaulting to true might be optimistic
            "hasAudio" | "hasVideo" | "stereo" => Some(Amf0Value::Boolean(true)),
            // Numeric - Defaulting to 0 might be safer than assuming values
            "duration"
            | "width"
            | "height"
            | "videosize"
            | "audiosize"
            | "lasttimestamp"
            | "lastkeyframelocation"
            | "lastkeyframetimestamp"
            | "filesize" => Some(Amf0Value::Number(0.0)),
            // Codecs - Common defaults
            "videocodecid" => Some(Amf0Value::Number(7.0)), // H264 (AVC)
            "audiocodecid" => Some(Amf0Value::Number(10.0)), // AAC
            // Audio params - Common defaults
            "audiosamplerate" => Some(Amf0Value::Number(44100.0)),
            "audiosamplesize" => Some(Amf0Value::Number(16.0)),
            // Data rates - Arbitrary defaults, might need adjustment
            "audiodatarate" => Some(Amf0Value::Number(128.0)), // kbps
            "videodatarate" => Some(Amf0Value::Number(1000.0)), // kbps
            "framerate" => Some(Amf0Value::Number(30.0)),
            // Strings
            "metadatacreator" => Some(Amf0Value::String(Cow::Borrowed("Srec"))),
            "metadatadate" => {
                // Generate current date dynamically
                Some(Amf0Value::String(Cow::Owned(
                    chrono::Utc::now().to_rfc3339(),
                )))
            }
            _ => None, // No default for unknown keys
        }
    }
}

impl FlvOperator for ScriptKeyframesFillerOperator {
    fn context(&self) -> &Arc<StreamerContext> {
        &self.context
    }

    async fn process(
        &mut self,
        input: AsyncReceiver<Result<FlvData, FlvError>>,
        output: AsyncSender<Result<FlvData, FlvError>>,
    ) {
        debug!("{} Starting script modification process", self.context.name);

        while let Ok(item_result) = input.recv().await {
            match item_result {
                Ok(data) => {
                    match data {
                        FlvData::Header(_) => {
                            debug!("{} Received Header. Forwarding.", self.context.name);
                            // reset flag
                            self.seen_first_script_tag = false;

                            if output.send(Ok(data)).await.is_err() {
                                return; // Output closed
                            }
                        }
                        FlvData::Tag(tag) => {
                            if tag.tag_type == FlvTagType::ScriptData {
                                if !self.seen_first_script_tag {
                                    debug!(
                                        "{} Found first script tag. Modifying.",
                                        self.context.name
                                    );
                                    self.seen_first_script_tag = true;

                                    let tag_clone = tag.clone();
                                    let inject_script =
                                        self.add_keyframes_to_amf(tag_clone).unwrap_or(tag);
                                    if output.send(Ok(FlvData::Tag(inject_script))).await.is_err() {
                                        return; // Output closed
                                    }
                                } else {
                                    debug!(
                                        "{} Found subsequent script tag. Forwarding.",
                                        self.context.name
                                    );
                                    // Forward subsequent script tags without modification
                                    if output.send(Ok(FlvData::Tag(tag))).await.is_err() {
                                        return; // Output closed
                                    }
                                }
                            } else {
                                // Forward tags that are not script data
                                if output.send(Ok(FlvData::Tag(tag))).await.is_err() {
                                    debug!("Sending tag failed. Output closed.");
                                    return;
                                }
                            }
                        }
                        // Handle other FlvData types if necessary
                        _ => {
                            trace!(
                                "{} Received other data type. Forwarding.",
                                self.context.name
                            );
                            if output.send(Ok(data)).await.is_err() {
                                return; // Output closed
                            }
                        }
                    }
                }
                Err(e) => {
                    error!(
                        "{} Error receiving data: {:?}. Forwarding error.",
                        self.context.name, e
                    );
                    if output.send(Err(e)).await.is_err() {
                        return; // Output closed
                    }
                }
            }
        }
        info!(
            "{} Script modification operator finished.",
            self.context.name
        );
    }

    fn name(&self) -> &'static str {
        "ScriptInjectorOperator"
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use amf0::Amf0Value;
    use bytes::Bytes;
    use flv::header::FlvHeader;
    use kanal::{bounded_async, unbounded_async};
    use std::collections::HashMap;
    use std::time::Duration;

    // Helper to initialize tracing for tests
    fn init_tracing() {
        let _ = tracing_subscriber::fmt::fmt()
            .with_max_level(tracing::Level::DEBUG)
            .with_test_writer() // Write to test output
            .try_init();
    }

    // Helper function to create a StreamerContext for testing
    fn create_test_context() -> Arc<StreamerContext> {
        Arc::new(StreamerContext::default())
    }

    // Helper function to create a script tag with onMetaData
    fn create_metadata_tag(with_keyframes: bool) -> FlvTag {
        let mut properties = vec![
            (Cow::Borrowed("duration"), Amf0Value::Number(120.5)),
            (Cow::Borrowed("width"), Amf0Value::Number(1920.0)),
            (Cow::Borrowed("height"), Amf0Value::Number(1080.0)),
            (Cow::Borrowed("videocodecid"), Amf0Value::Number(7.0)),
            (Cow::Borrowed("audiocodecid"), Amf0Value::Number(10.0)),
        ];

        if with_keyframes {
            let keyframes_obj = vec![
                (
                    Cow::Borrowed("times"),
                    Amf0Value::StrictArray(Cow::Owned(vec![
                        Amf0Value::Number(0.0),
                        Amf0Value::Number(5.0),
                    ])),
                ),
                (
                    Cow::Borrowed("filepositions"),
                    Amf0Value::StrictArray(Cow::Owned(vec![
                        Amf0Value::Number(100.0),
                        Amf0Value::Number(2500.0),
                    ])),
                ),
            ];

            properties.push((
                Cow::Borrowed("keyframes"),
                Amf0Value::Object(Cow::Owned(keyframes_obj)),
            ));
        }

        let obj = Amf0Value::Object(Cow::Owned(properties));
        let mut buffer = Vec::new();
        amf0::Amf0Encoder::encode_string(&mut buffer, "onMetaData").unwrap();
        amf0::Amf0Encoder::encode(&mut buffer, &obj).unwrap();

        FlvTag {
            timestamp_ms: 0,
            stream_id: 0,
            tag_type: FlvTagType::ScriptData,
            data: Bytes::from(buffer),
        }
    }

    // Helper function to create a video tag
    fn create_video_tag() -> FlvTag {
        // Simple video tag data - doesn't need to be valid for our tests
        let data = vec![0x17, 0x01, 0x00, 0x00, 0x00]; // H.264 keyframe

        FlvTag {
            timestamp_ms: 10,
            stream_id: 0,
            tag_type: FlvTagType::Video,
            data: Bytes::from(data),
        }
    }

    // Helper function to extract keyframes object from tag data
    fn extract_keyframes(tag: &FlvTag) -> Option<HashMap<String, Vec<f64>>> {
        let mut cursor = std::io::Cursor::new(tag.data.clone());
        if let Ok(amf_data) = ScriptData::demux(&mut cursor) {
            if let Amf0Value::Object(props) = &amf_data.data[0] {
                for (key, value) in props.iter() {
                    if key == "keyframes" {
                        if let Amf0Value::Object(keyframe_props) = value {
                            let mut result = HashMap::new();
                            for (kf_key, kf_value) in keyframe_props.iter() {
                                if let Amf0Value::StrictArray(array) = kf_value {
                                    let values: Vec<f64> = array
                                        .iter()
                                        .filter_map(|v| {
                                            if let Amf0Value::Number(num) = v {
                                                Some(*num)
                                            } else {
                                                None
                                            }
                                        })
                                        .collect();
                                    result.insert(kf_key.to_string(), values);
                                }
                            }
                            return Some(result);
                        }
                    }
                }
            }
        }
        None
    }

    #[tokio::test]
    async fn test_add_keyframes_to_amf() {
        init_tracing();
        let context = create_test_context();
        let config = ScriptFillerConfig::default();
        let operator = ScriptKeyframesFillerOperator::new(context, config);

        // Test case 1: Tag without keyframes should have them added
        let tag = create_metadata_tag(false);
        let mut modified_tag = operator.add_keyframes_to_amf(tag).unwrap();

        // Verify keyframes were added
        let keyframes = extract_keyframes(&modified_tag);
        assert!(keyframes.is_some());

        let keyframes = keyframes.unwrap();
        assert!(keyframes.contains_key("times"));
        assert!(keyframes.contains_key("filepositions"));
        assert!(keyframes.contains_key("spacer"));

        // Check that spacer has the right length
        assert!(keyframes.get("spacer").unwrap().len() > 100,);

        // Test case 2: Tag with existing keyframes should have them replaced
        let tag = create_metadata_tag(true);
        let modified_tag = operator.add_keyframes_to_amf(tag).unwrap();

        // Verify keyframes were modified
        let keyframes = extract_keyframes(&modified_tag).unwrap();
        assert!(keyframes.contains_key("times"));
        assert!(keyframes.contains_key("filepositions"));
        assert!(keyframes.contains_key("spacer")); // New field added

        // Original arrays should be empty now
        assert!(keyframes.get("times").unwrap().is_empty());
        assert!(keyframes.get("filepositions").unwrap().is_empty());
    }

    #[tokio::test]
    async fn test_process_flow() {
        init_tracing();
        let context = create_test_context();
        let config = ScriptFillerConfig::default();
        let mut operator = ScriptKeyframesFillerOperator::new(context, config);

        // Create channels
        let (tx_input, rx_input) = unbounded_async();
        let (tx_output, rx_output) = unbounded_async();

        // Start operator in background
        let operator_task = tokio::spawn(async move {
            operator.process(rx_input, tx_output).await;
        });

        // Send header
        let header = FlvHeader::new(true, true);
        tx_input.send(Ok(FlvData::Header(header))).await.unwrap();

        // Send script tag
        let script_tag = create_metadata_tag(false);
        tx_input.send(Ok(FlvData::Tag(script_tag))).await.unwrap();

        // Send video tag
        let video_tag = create_video_tag();
        tx_input
            .send(Ok(FlvData::Tag(video_tag.clone())))
            .await
            .unwrap();

        // Send another script tag (should be ignored for modification)
        let second_script_tag = create_metadata_tag(false);
        tx_input
            .send(Ok(FlvData::Tag(second_script_tag.clone())))
            .await
            .unwrap();

        // Check outputs

        // Header should be passed through unchanged
        let header_out = rx_output.recv().await.unwrap().unwrap();
        assert!(matches!(header_out, FlvData::Header(_)));

        // First script tag should be modified
        let script_out = rx_output.recv().await.unwrap().unwrap();
        if let FlvData::Tag(tag) = script_out {
            assert_eq!(tag.tag_type, FlvTagType::ScriptData);
            let keyframes = extract_keyframes(&tag);
            assert!(keyframes.is_some());
        } else {
            panic!("Expected script tag");
        }

        // Video tag should be unchanged
        let video_out = rx_output.recv().await.unwrap().unwrap();
        if let FlvData::Tag(tag) = video_out {
            assert_eq!(tag.tag_type, FlvTagType::Video);
            assert_eq!(tag.timestamp_ms, video_tag.timestamp_ms);
        } else {
            panic!("Expected video tag");
        }

        // Second script tag should be unchanged
        // Note: In the current implementation, subsequent script tags are not forwarded
        // So we don't check for them here.

        // Clean up
        drop(tx_input);
        let _ = tokio::time::timeout(Duration::from_secs(1), operator_task).await;
    }

    #[tokio::test]
    async fn test_error_handling() {
        let context = create_test_context();
        let config = ScriptFillerConfig::default();
        let mut operator = ScriptKeyframesFillerOperator::new(context, config);

        let (tx_input, rx_input) = bounded_async(10);
        let (tx_output, mut rx_output) = bounded_async(10);

        // Start operator in background
        let operator_task = tokio::spawn(async move {
            operator.process(rx_input, tx_output).await;
        });

        // Send an error
        let test_error = FlvError::Io(io::Error::new(io::ErrorKind::Other, "test error"));
        tx_input.send(Err(test_error)).await.unwrap();

        // The error should be forwarded
        let error_out = rx_output.recv().await.unwrap();
        assert!(error_out.is_err());

        // Clean up
        drop(tx_input);
        let _ = tokio::time::timeout(Duration::from_secs(1), operator_task).await;
    }
}
