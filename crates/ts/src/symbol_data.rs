//! TypeScript-specific per-symbol metadata.
//!
//! This data is stored in `Symbol<TsSymbolData>.language_data` and carries
//! information that only makes sense for TypeScript/React components:
//! rendered components (JSX tree) and CSS class tokens.

use serde::{Deserialize, Serialize};

/// Per-symbol metadata for TypeScript/React components.
///
/// Stored in `Symbol<TsSymbolData>.language_data`. Contains:
/// - Which components this component renders internally (JSX tree)
/// - Which CSS class tokens this component uses (`styles.xxx` references)
#[derive(Debug, Clone, Default, PartialEq, Eq, Serialize, Deserialize)]
pub struct TsSymbolData {
    /// Components from the same package that this component renders internally
    /// in its JSX return tree. Determined by parsing the `.tsx` source file.
    ///
    /// Used for hierarchy inference: components in the same family that do NOT
    /// appear in this list are likely consumer-provided children.
    ///
    /// Only populated for Function/Variable/Constant symbols that represent
    /// React components with JSX render functions.
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub rendered_components: Vec<String>,

    /// CSS class tokens used by this component (e.g., `["inputGroup", "inputGroupItem"]`).
    /// Extracted from `styles.xxx` references in component source files.
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub css: Vec<String>,
}
