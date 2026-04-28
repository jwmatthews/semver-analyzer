//! Java analysis extensions for the SD pipeline.
//!
//! This is the concrete type for `Language::AnalysisExtensions`.
//! It carries the SD pipeline results through the orchestrator
//! and into rule generation.

use crate::sd_types::JavaSdPipelineResult;
use serde::{Deserialize, Serialize};

/// Java-specific analysis extensions.
///
/// Populated by `Java::run_extended_analysis()` and consumed by
/// `generate_sd_rules()` for Konveyor rule generation.
#[derive(Debug, Clone, Default, Serialize, Deserialize)]
pub struct JavaAnalysisExtensions {
    /// Source-level diff pipeline results.
    pub sd_result: Option<JavaSdPipelineResult>,
}
