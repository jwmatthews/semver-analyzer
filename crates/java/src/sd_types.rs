//! Java SD pipeline types.
//!
//! Defines the structured data types for the source-level diff pipeline:
//! - `JavaClassProfile` — per-class behavioral snapshot
//! - `JavaSourceChange` — deterministic AST-derived change
//! - `JavaSdPipelineResult` — full pipeline output

use serde::{Deserialize, Serialize};
use std::collections::HashMap;

// ── Per-class behavioral profile ────────────────────────────────────────

/// Behavioral profile extracted from a Java class/interface/enum/record.
///
/// Captures the aspects of a class that affect observable behavior,
/// beyond what the structural API surface (type signatures) covers.
#[derive(Debug, Clone, Default, Serialize, Deserialize)]
pub struct JavaClassProfile {
    /// Fully qualified class name (e.g., `com.example.Foo`).
    pub qualified_name: String,

    /// Simple class name.
    pub name: String,

    /// Source file path (relative to project root).
    pub file: String,

    /// Annotations on this class/interface.
    pub annotations: Vec<ProfileAnnotation>,

    /// Methods this class declares, with their behavioral metadata.
    pub methods: Vec<MethodProfile>,

    /// Constructor parameter types (ordered). Captures DI dependencies.
    pub constructor_params: Vec<String>,

    /// Interfaces implemented by this class.
    pub implements: Vec<String>,

    /// Superclass (if any).
    pub extends: Option<String>,

    /// Whether this class is final.
    pub is_final: bool,

    /// Whether this class is sealed.
    pub is_sealed: bool,

    /// Whether this class is abstract.
    pub is_abstract: bool,

    /// Permitted subtypes (for sealed classes).
    pub permits: Vec<String>,

    /// Whether this class implements Serializable.
    pub is_serializable: bool,

    /// Fields on this class (for serialization analysis).
    pub fields: Vec<FieldProfile>,

    /// Module this class belongs to (from module-info.java, if any).
    pub module_name: Option<String>,
}

/// An annotation on a class or method, with parameters.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct ProfileAnnotation {
    pub name: String,
    pub qualified_name: Option<String>,
    pub attributes: Vec<(String, String)>,
}

/// Behavioral profile of a single method.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct MethodProfile {
    pub name: String,
    pub qualified_name: String,
    pub is_synchronized: bool,
    pub is_native: bool,
    pub is_override: bool,
    pub is_default: bool,
    pub is_abstract: bool,
    pub thrown_exceptions: Vec<String>,
    pub annotations: Vec<ProfileAnnotation>,
    /// Methods this method calls (delegation targets).
    pub delegations: Vec<String>,
    /// Return type (for behavioral change detection).
    pub return_type: Option<String>,
    /// Parameter types.
    pub param_types: Vec<String>,
}

/// Field profile for serialization analysis.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct FieldProfile {
    pub name: String,
    pub field_type: String,
    pub is_transient: bool,
    pub is_volatile: bool,
    pub is_static: bool,
    pub is_final: bool,
}

// ── Source-level changes ────────────────────────────────────────────────

/// A single deterministic source-level change between two versions.
///
/// Each change is derived from AST analysis, not heuristics or LLM.
/// The category determines what kind of behavioral impact the change has.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct JavaSourceChange {
    /// Fully qualified class name affected.
    pub class_name: String,

    /// Category of the change.
    pub category: JavaSourceCategory,

    /// Human-readable description.
    pub description: String,

    /// Value before the change (if applicable).
    #[serde(skip_serializing_if = "Option::is_none")]
    pub old_value: Option<String>,

    /// Value after the change (if applicable).
    #[serde(skip_serializing_if = "Option::is_none")]
    pub new_value: Option<String>,

    /// Whether this change is breaking.
    pub is_breaking: bool,

    /// Affected method name (for method-level changes).
    #[serde(skip_serializing_if = "Option::is_none")]
    pub method: Option<String>,

    /// Dependency chain for transitive changes.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub dependency_chain: Option<Vec<String>>,
}

/// Categories of source-level changes.
///
/// Each category represents a deterministic, AST-derived fact about
/// what changed between versions — no LLM, no confidence scores.
#[derive(Debug, Clone, PartialEq, Eq, Hash, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum JavaSourceCategory {
    /// Annotation removed from a class or method (e.g., `@Deprecated` removed).
    AnnotationRemoved,
    /// Annotation added to a class or method.
    AnnotationAdded,
    /// Annotation parameters changed (e.g., `@Retention(RUNTIME)` → `@Retention(SOURCE)`).
    AnnotationChanged,
    /// Method now calls different methods (behavioral delegation change).
    DelegationChanged,
    /// New checked exception added to a method's throws clause.
    ExceptionAdded,
    /// Checked exception removed from throws clause.
    ExceptionRemoved,
    /// `synchronized` modifier removed from a method.
    SynchronizationRemoved,
    /// `synchronized` modifier added to a method.
    SynchronizationAdded,
    /// Field added to a Serializable class (may break deserialization).
    SerializationFieldAdded,
    /// Field removed from a Serializable class.
    SerializationFieldRemoved,
    /// Field type changed on a Serializable class.
    SerializationFieldTypeChanged,
    /// `transient` modifier added/removed on a Serializable field.
    TransientChanged,
    /// `@Override` method removed (behavior reverts to parent).
    OverrideRemoved,
    /// `@Override` method added.
    OverrideAdded,
    /// Constructor parameter types changed (DI wiring affected).
    ConstructorDependencyChanged,
    /// Module `exports` directive removed (package inaccessible).
    ModuleExportRemoved,
    /// Module `exports` directive added.
    ModuleExportAdded,
    /// Module `requires` directive changed.
    ModuleRequiresChanged,
    /// Class became final (no subclassing).
    FinalAdded,
    /// Class is no longer final.
    FinalRemoved,
    /// Class became sealed.
    SealedChanged,
    /// Inheritance changed (`extends` or `implements`).
    InheritanceChanged,
    /// `native` modifier removed (JNI consumers break).
    NativeRemoved,
}

// ── Pipeline result ─────────────────────────────────────────────────────

/// Full output of the Java SD pipeline.
#[derive(Debug, Clone, Default, Serialize, Deserialize)]
pub struct JavaSdPipelineResult {
    /// Individual source-level changes detected.
    pub source_level_changes: Vec<JavaSourceChange>,

    /// Class profiles at the old version (for changed classes).
    #[serde(skip)]
    pub old_profiles: HashMap<String, JavaClassProfile>,

    /// Class profiles at the new version (all classes).
    #[serde(skip)]
    pub new_profiles: HashMap<String, JavaClassProfile>,

    /// Module-level changes (exports/requires/opens).
    pub module_changes: Vec<JavaSourceChange>,

    /// Inheritance tree summary (new version).
    pub inheritance_summary: Vec<InheritanceEntry>,
}

/// An entry in the inheritance tree summary.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct InheritanceEntry {
    pub class_name: String,
    pub extends: Option<String>,
    pub implements: Vec<String>,
    pub is_final: bool,
    pub is_sealed: bool,
    pub subclasses: Vec<String>,
}

// ── Module changes ──────────────────────────────────────────────────────

/// A change to a Java module directive.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ModuleDirectiveChange {
    pub module_name: String,
    pub directive_kind: String,
    pub old_value: Option<String>,
    pub new_value: Option<String>,
    pub is_breaking: bool,
    pub description: String,
}
