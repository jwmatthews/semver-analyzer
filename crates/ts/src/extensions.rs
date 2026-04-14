//! TypeScript-specific analysis extensions.
//!
//! `TsAnalysisExtensions` wraps the SD pipeline results and hierarchy data
//! that flow through `AnalysisReport<TypeScript>` and `AnalysisResult<TypeScript>`.
//! It replaces the concrete `sd_result` and `hierarchy_deltas` fields that were
//! previously on the language-agnostic report/result types.

use semver_analyzer_core::{ExpectedChild, HierarchyDelta};
use serde::{Deserialize, Serialize};
use std::collections::HashMap;

use crate::sd_types::SdPipelineResult;

/// Language-specific analysis extensions for TypeScript.
///
/// Contains SD pipeline results (composition trees, conformance checks,
/// source-level changes) and hierarchy deltas from LLM inference.
///
/// Serialized with `#[serde(flatten)]` on `AnalysisReport`/`AnalysisResult`,
/// so fields appear at the top level in JSON output for backward compatibility.
#[derive(Debug, Clone, Default, Serialize, Deserialize)]
pub struct TsAnalysisExtensions {
    /// SD (Source-Level Diff) pipeline results.
    /// Populated by default; None when `--behavioral` is used.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub sd_result: Option<SdPipelineResult>,

    /// Hierarchy changes between versions, computed by diffing LLM-inferred
    /// component hierarchies from both refs. Each entry describes how a
    /// component's expected children changed (added/removed children,
    /// migrated props). None when --no-llm is set.
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub hierarchy_deltas: Vec<HierarchyDelta>,

    /// Per-family component hierarchies for the new version.
    /// Maps family name → (component name → expected children).
    /// Not serialized into reports (only used during pipeline execution).
    #[serde(default, skip_serializing_if = "HashMap::is_empty")]
    pub new_hierarchies: HashMap<String, HashMap<String, Vec<ExpectedChild>>>,
}
