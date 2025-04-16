//! Pipeline operators for FLV stream processing
//!
//! This module provides a collection of operators for processing FLV (Flash Video) streams.
//! These operators can be combined into a pipeline to perform various transformations and
//! validations on FLV data.

pub mod defragment;
pub mod gop_sort;
pub mod header_check;
pub mod limit;
pub mod script_filler;
pub mod script_filter;
pub mod split;
pub mod time_consistency;
pub mod timing_repair;

use crate::context::StreamerContext;
use flv::data::FlvData;
use flv::error::FlvError;
use std::sync::Arc;

/// Trait for processing FLV data in a synchronous pipeline
///
/// This trait provides a synchronous interface for processing FLV data through a pipeline.
pub trait FlvProcessor {
    /// Process input data and produce output
    ///
    /// Processes a single FLV data item and calls the output function for each
    /// produced item. Returns a result indicating whether processing should continue.
    fn process(
        &mut self,
        input: FlvData,
        output: &mut dyn FnMut(FlvData) -> Result<(), FlvError>,
    ) -> Result<(), FlvError>;

    /// Called when processing is complete to clean up or flush buffers
    ///
    /// This method is called at the end of processing to allow operators
    /// to flush any buffered data before the pipeline terminates.
    fn finish(
        &mut self,
        output: &mut dyn FnMut(FlvData) -> Result<(), FlvError>,
    ) -> Result<(), FlvError>;

    /// Get the name of this operator for logging and debugging
    fn name(&self) -> &'static str;
}

/// A pipeline for processing FLV data through a series of operators
///
/// This struct represents a full processing pipeline composed of multiple
/// FlvProcessor operators that process FLV data sequentially.
pub struct NFlvPipeline {
    processors: Vec<Box<dyn FlvProcessor>>,
    #[allow(dead_code)]
    context: Arc<StreamerContext>,
}

impl NFlvPipeline {
    /// Create a new empty pipeline with the given context
    pub fn new(context: Arc<StreamerContext>) -> Self {
        Self {
            processors: Vec::new(),
            context,
        }
    }

    /// Add a processor to the end of the pipeline
    ///
    /// Returns self for method chaining.
    pub fn add_processor<P: FlvProcessor + 'static>(mut self, processor: P) -> Self {
        self.processors.push(Box::new(processor));
        self
    }

    /// Process all input through the pipeline
    ///
    /// Takes an iterator of input FLV data and a function to handle output data.
    /// Returns an error if any processor in the pipeline fails.
    pub fn process(
        mut self,
        input: impl Iterator<Item = Result<FlvData, FlvError>>,
        output: &mut dyn FnMut(Result<FlvData, FlvError>),
    ) -> Result<(), FlvError> {
        // Recursive processing function
        fn process_inner(
            processors: &mut [Box<dyn FlvProcessor>],
            data: FlvData,
            output: &mut dyn FnMut(FlvData) -> Result<(), FlvError>,
        ) -> Result<(), FlvError> {
            if let Some((first, rest)) = processors.split_first_mut() {
                let mut intermediate_output = |data| process_inner(rest, data, output);
                first.process(data, &mut intermediate_output)
            } else {
                output(data)
            }
        }

        // Convert external output function to internal format
        let mut internal_output = |data: FlvData| {
            output(Ok(data));
            Ok(())
        };

        // Process the input stream
        for item in input {
            let data = item?;
            process_inner(&mut self.processors, data, &mut internal_output)?;
        }

        // Finalize processing
        let mut processors = &mut self.processors[..];
        while !processors.is_empty() {
            let (current, rest) = processors.split_first_mut().unwrap();
            let mut output_fn = |data: FlvData| process_inner(rest, data, &mut internal_output);
            current.finish(&mut output_fn)?;
            processors = rest;
        }

        Ok(())
    }
}

// Re-export common operators
pub use defragment::DefragmentOperator;
pub use gop_sort::GopSortOperator;
pub use header_check::HeaderCheckOperator;
pub use limit::LimitOperator;
pub use script_filler::ScriptKeyframesFillerOperator;
pub use script_filter::ScriptFilterOperator;
pub use split::SplitOperator;
pub use time_consistency::{ContinuityMode, TimeConsistencyOperator};
pub use timing_repair::{RepairStrategy, TimingRepairConfig, TimingRepairOperator};
