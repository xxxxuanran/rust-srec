//! Pipeline operators for FLV stream processing

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
use kanal::{AsyncReceiver, AsyncSender};
use std::sync::Arc;

/// Common trait for all FLV stream operators
pub trait FlvOperator {
    /// Get the operator's context
    fn context(&self) -> &Arc<StreamerContext>;

    /// Process the FLV stream asynchronously
    fn process(
        &mut self,
        input: AsyncReceiver<Result<FlvData, FlvError>>,
        output: AsyncSender<Result<FlvData, FlvError>>,
    ) -> impl std::future::Future<Output = ()> + Send;

    /// Get the name of this operator for logging and debugging
    fn name(&self) -> &'static str;

    /// Process a single FLV data item (optional implementation for stateless operators)
    fn process_item(&mut self, item: Result<FlvData, FlvError>) -> Result<FlvData, FlvError> {
        // Default implementation just returns the item unchanged
        item
    }
}

pub trait FlvProcessor {
    /// 处理输入并产生输出，返回是否继续处理
    fn process(
        &mut self,
        input: FlvData,
        output: &mut dyn FnMut(FlvData) -> Result<(), FlvError>,
    ) -> Result<(), FlvError>;

    /// 处理结束时调用，用于清理或刷新缓冲
    fn finish(
        &mut self,
        output: &mut dyn FnMut(FlvData) -> Result<(), FlvError>,
    ) -> Result<(), FlvError>;
    fn name(&self) -> &'static str;
}

pub struct NFlvPipeline {
    processors: Vec<Box<dyn FlvProcessor>>,
    context: Arc<StreamerContext>,
}

impl NFlvPipeline {
    pub fn new(context: Arc<StreamerContext>) -> Self {
        Self {
            processors: Vec::new(),
            context: context,
        }
    }

    pub fn add_processor<P: FlvProcessor + 'static>(mut self, processor: P) -> Self {
        self.processors.push(Box::new(processor));
        self
    }

    pub fn process(
        mut self,
        input: impl Iterator<Item = Result<FlvData, FlvError>>,
        output: &mut dyn FnMut(Result<FlvData, FlvError>),
    ) -> Result<(), FlvError> {
        // 递归处理函数
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

        // 转换外部输出函数为内部格式
        let mut internal_output = |data: FlvData| {
            output(Ok(data));
            Ok(())
        };

        // 处理输入流
        for item in input {
            let data = item?;
            process_inner(&mut self.processors, data, &mut internal_output)?;
        }

        // 完成处理
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
