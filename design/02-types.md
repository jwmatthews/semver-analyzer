# Core Types Reference

Every public struct and enum in `crates/core/src/types/`. Grouped by module.

---

## Module: `surface` (`crates/core/src/types/surface.rs`)

### `ApiSurface<M>`

Language-agnostic public API surface extracted from source code at a git ref.
Used by the TD (Top-Down) pipeline.

```rust
// surface.rs:28
pub struct ApiSurface<M: Default + Clone + PartialEq = ()> {
    pub symbols: Vec<Symbol<M>>,
}
```

### `Symbol<M>`

A single exported symbol in the API surface. Symbols form a tree: a `Class`
symbol has `members` which are themselves `Symbol<M>` values.

```rust
// surface.rs:55
pub struct Symbol<M: Default + Clone + PartialEq = ()> {
    pub name: String,                       // Simple name (e.g., "createUser")
    pub qualified_name: String,             // Fully qualified (e.g., "src/api/users.createUser")
    pub kind: SymbolKind,                   // What kind of symbol
    pub visibility: Visibility,             // Export visibility level
    pub file: PathBuf,                      // Source file
    pub package: Option<String>,            // Distribution/dependency identity (e.g., npm package name)
    pub import_path: Option<String>,        // Consumer-facing import specifier (e.g., subpath export)
    pub line: usize,                        // Line number (1-indexed)
    pub signature: Option<Signature>,       // Function/method signature (None for non-callable)
    pub extends: Option<String>,            // Parent class (`extends` clause)
    pub implements: Vec<String>,            // Implemented interfaces
    pub is_abstract: bool,                  // Whether abstract (class or method)
    pub type_dependencies: Vec<String>,     // Types referenced in signature
    pub is_readonly: bool,                  // Whether readonly
    pub is_static: bool,                    // Whether static
    pub accessor_kind: Option<AccessorKind>,// Get/set accessor kind
    pub members: Vec<Symbol<M>>,            // Child members (methods, properties, enum variants)
    pub language_data: M,                   // Per-symbol language-specific metadata
}
```

20 fields total. `language_data` is `TsSymbolData` for TypeScript, `()` for core/tests.

### `SymbolKind`

15 variants.

```rust
// surface.rs:222
pub enum SymbolKind {
    Function,
    Method,
    Class,
    Struct,          // Value type (Go, C#)
    Interface,
    TypeAlias,
    Enum,
    EnumMember,
    Constant,
    Variable,
    Property,
    Constructor,
    GetAccessor,     // Still present -- NOT removed
    SetAccessor,     // Still present -- NOT removed
    Namespace,
}
```

Serde: `#[serde(rename_all = "snake_case")]`.

### `Visibility`

5 variants. Default: `Public`.

```rust
// surface.rs:247
pub enum Visibility {
    Exported,    // Module-level export (JS/TS `export`, Python `__all__`, Rust `pub`)
    Public,      // Public member (default)
    Protected,   // Subclass-accessible (Java/C# `protected`, Python `_prefix`)
    Internal,    // Module-internal (not exported)
    Private,     // Explicitly private (`private` keyword or `#field`)
}
```

Serde: `#[serde(rename_all = "snake_case")]`.

### `AccessorKind`

```rust
// surface.rs:267
pub enum AccessorKind {
    Get,
    Set,
    GetSet,
}
```

### `Signature`

Function or method signature.

```rust
// surface.rs:276
pub struct Signature {
    pub parameters: Vec<Parameter>,
    pub return_type: Option<String>,           // Canonicalized (e.g., "Promise<User>")
    pub type_parameters: Vec<TypeParameter>,
    pub is_async: bool,
}
```

### `Parameter`

```rust
// surface.rs:306
pub struct Parameter {
    pub name: String,
    pub type_annotation: Option<String>,
    pub optional: bool,                  // Whether optional (`param?: Type`)
    pub has_default: bool,               // Whether has a default value
    pub default_value: Option<String>,   // Actual default expression (e.g., "10", "'hello'")
    pub is_variadic: bool,               // Rest/variadic parameter (`...args`) -- NOT `is_rest`
}
```

### `TypeParameter`

Generic type parameter declaration.

```rust
// surface.rs:293
pub struct TypeParameter {
    pub name: String,                // e.g., "T"
    pub constraint: Option<String>,  // e.g., "Serializable" from `T extends Serializable`
    pub default: Option<String>,     // e.g., "unknown" from `T = unknown`
}
```

---

## Module: `report` (`crates/core/src/types/report.rs`)

### `EmptyExtensions`

Empty analysis extensions for languages without extended analysis. Serializes
as `{}`.

```rust
// report.rs:39
pub struct EmptyExtensions {}
```

### `AnalysisReport<L>`

Top-level analysis report (v2 harness format). 11 fields.

```rust
// report.rs:45
pub struct AnalysisReport<L: Language> {
    pub repository: PathBuf,
    pub comparison: Comparison,
    pub summary: Summary,
    pub changes: Vec<FileChanges<L>>,
    pub manifest_changes: Vec<ManifestChange<L>>,
    pub added_files: Vec<PathBuf>,
    pub packages: Vec<PackageChanges<L>>,
    pub member_renames: HashMap<String, String>,
    pub inferred_rename_patterns: Option<InferredRenamePatterns>,
    pub extensions: L::AnalysisExtensions,       // #[serde(flatten)]
    pub metadata: AnalysisMetadata,
}
```

`extensions` is flattened into the parent JSON object.

### `Comparison`

Git comparison metadata.

```rust
// report.rs:113
pub struct Comparison {
    pub from_ref: String,
    pub to_ref: String,
    pub from_sha: String,
    pub to_sha: String,
    pub commit_count: usize,
    pub analysis_timestamp: String,
}
```

### `Summary`

```rust
// report.rs:124
pub struct Summary {
    pub total_breaking_changes: usize,
    pub breaking_api_changes: usize,
    pub breaking_behavioral_changes: usize,
    pub files_with_breaking_changes: usize,
}
```

### `FileChanges<L>`

All breaking changes within a single file.

```rust
// report.rs:134
pub struct FileChanges<L: Language> {
    pub file: PathBuf,
    pub status: FileStatus,
    pub renamed_from: Option<PathBuf>,
    pub breaking_api_changes: Vec<ApiChange>,
    pub breaking_behavioral_changes: Vec<BehavioralChange<L>>,
    pub container_changes: Vec<ContainerChange>,
}
```

### `FileStatus`

```rust
// report.rs:160
pub enum FileStatus {
    Added,
    Modified,
    Deleted,
    Renamed,
}
```

### `ApiChange`

A breaking API change detected by structural analysis (TD pipeline).

```rust
// report.rs:174
pub struct ApiChange {
    pub symbol: String,                         // "TypeName" or "TypeName.memberName"
    pub qualified_name: String,                 // Fully qualified
    pub kind: ApiChangeKind,
    pub change: ApiChangeType,
    pub before: Option<String>,
    pub after: Option<String>,
    pub description: String,
    pub migration_target: Option<MigrationTarget>,
    pub removal_disposition: Option<RemovalDisposition>,
}
```

### `ApiChangeKind`

```rust
// report.rs:214
pub enum ApiChangeKind {
    Function,
    Method,
    Class,
    Struct,          // #[serde(rename = "struct")]
    Interface,
    Trait,           // #[serde(rename = "trait")]
    TypeAlias,
    Constant,
    Enum,            // #[serde(rename = "enum")]
    Constructor,
    Field,
    Property,
    ModuleExport,
}
```

Serde: `#[serde(rename_all = "snake_case")]` with manual renames for Rust keywords.

### `ApiChangeType`

```rust
// report.rs:257
pub enum ApiChangeType {
    Removed,
    SignatureChanged,
    TypeChanged,
    VisibilityChanged,
    Renamed,
}
```

### `BehavioralChange<L>`

A behavioral change detected by BU analysis.

```rust
// report.rs:268
pub struct BehavioralChange<L: Language> {
    pub symbol: String,
    pub kind: BehavioralChangeKind,
    pub category: Option<L::Category>,
    pub description: String,
    pub source_file: Option<String>,            // #[serde(skip)]
    pub confidence: Option<f64>,                // NOT bare f64
    pub evidence_type: Option<EvidenceType>,    // NOT `evidence: L::Evidence`
    pub referenced_symbols: Vec<String>,
    pub is_internal_only: Option<bool>,          // NOT bare bool
}
```

### `BehavioralChangeKind`

```rust
// report.rs:312
pub enum BehavioralChangeKind {
    Function,
    Method,
    Class,
    Module,
}
```

### `ContainerChange`

Containment/nesting structure change between versions.

```rust
// report.rs:325
pub struct ContainerChange {
    pub symbol: String,
    pub old_container: Option<String>,
    pub new_container: Option<String>,
    pub description: String,
}
```

### `PackageChanges<L>`

All changes within a single package.

```rust
// report.rs:345
pub struct PackageChanges<L: Language> {
    pub name: String,
    pub old_version: Option<String>,
    pub new_version: Option<String>,
    pub type_summaries: Vec<TypeSummary<L>>,
    pub constants: Vec<ConstantGroup>,
    pub added_exports: Vec<AddedExport>,
}
```

### `TypeSummary<L>`

Pre-aggregated summary of all changes to a single type.

```rust
// report.rs:378
pub struct TypeSummary<L: Language> {
    pub name: String,
    pub definition_name: String,
    pub status: TypeStatus,
    pub member_summary: MemberSummary,
    pub removed_members: Vec<RemovedMember>,
    pub type_changes: Vec<TypeChange>,
    pub migration_target: Option<MigrationTarget>,
    pub behavioral_changes: Vec<BehavioralChange<L>>,
    pub language_data: L::ReportData,            // #[serde(flatten)]
    pub source_files: Vec<PathBuf>,
}
```

### `TypeStatus`

```rust
// report.rs:423
pub enum TypeStatus {
    Modified,
    Removed,
    Added,
}
```

### `MemberSummary`

Aggregated member-level change counts for a type.

```rust
// report.rs:435
pub struct MemberSummary {
    pub total: usize,
    pub removed: usize,
    pub renamed: usize,
    pub type_changed: usize,
    pub added: usize,
    pub removal_ratio: f64,     // 0.0 to 1.0
}
```

### `RemovedMember`

```rust
// report.rs:454
pub struct RemovedMember {
    pub name: String,
    pub old_type: Option<String>,
    pub removal_disposition: Option<RemovalDisposition>,
}
```

### `RemovalDisposition`

Why a member was removed and where its functionality went. Internally tagged
with `#[serde(tag = "type", rename_all = "snake_case")]`.

```rust
// report.rs:469
pub enum RemovalDisposition {
    MovedToRelatedType {
        target_type: String,
        mechanism: String,       // e.g., "prop", "children", "parameter", "field"
    },
    ReplacedByMember {
        new_member: String,
    },
    MadeAutomatic,
    TrulyRemoved,
}
```

### `TypeChange`

A property whose type changed.

```rust
// report.rs:492
pub struct TypeChange {
    pub property: String,
    pub before: Option<String>,
    pub after: Option<String>,
}
```

### `ExpectedChild`

An expected direct child type, derived from LLM hierarchy inference.

```rust
// report.rs:513
pub struct ExpectedChild {
    pub name: String,
    pub required: bool,
    pub mechanism: String,           // "child" (default) or "prop"
    pub prop_name: Option<String>,   // When mechanism is "prop"
}
```

### `HierarchyDelta`

A change in the component hierarchy between versions.

```rust
// report.rs:558
pub struct HierarchyDelta {
    pub component: String,
    pub added_children: Vec<ExpectedChild>,
    pub removed_children: Vec<String>,
    pub migrated_members: Vec<MigratedMember>,
    pub source_package: Option<String>,
    pub migration_target: Option<MigrationTarget>,
}
```

### `MigratedMember`

A member that migrated from a parent type to a child type.

```rust
// report.rs:586
pub struct MigratedMember {
    pub member_name: String,
    pub target_child: String,
    pub target_member_name: Option<String>,
}
```

### `FamilyHierarchy`

```rust
// report.rs:602
pub struct FamilyHierarchy {
    pub components: HashMap<String, Vec<ExpectedChild>>,
}
```

### `ConstantGroup`

Pre-grouped bulk constant/token changes within a package.

```rust
// report.rs:613
pub struct ConstantGroup {
    pub change_type: ApiChangeType,
    pub count: usize,
    pub symbols: Vec<String>,
    pub common_prefix_pattern: String,
    pub strategy_hint: String,
    pub suffix_renames: Vec<SuffixRename>,
}
```

### `SuffixRename`

```rust
// report.rs:634
pub struct SuffixRename {
    pub from: String,
    pub to: String,
}
```

### `AddedExport`

A symbol that was added (newly exported) in the new version.

```rust
// report.rs:642
pub struct AddedExport {
    pub name: String,
    pub qualified_name: String,
    pub package: String,
}
```

### `StructuralChange`

A structural change detected by the diff engine. Internal representation
converted to `ApiChange` for output.

```rust
// report.rs:662
pub struct StructuralChange {
    pub symbol: String,
    pub qualified_name: String,
    pub kind: SymbolKind,
    pub package: Option<String>,
    pub change_type: StructuralChangeType,
    pub before: Option<String>,
    pub after: Option<String>,
    pub description: String,
    pub is_breaking: bool,
    pub impact: Option<ImpactAnalysis>,
    pub migration_target: Option<MigrationTarget>,
}
```

### `StructuralChangeType`

5 lifecycle variants, each carrying a `ChangeSubject`.

```rust
// report.rs:709
pub enum StructuralChangeType {
    Added(ChangeSubject),
    Removed(ChangeSubject),
    Changed(ChangeSubject),
    Renamed { from: ChangeSubject, to: ChangeSubject },
    Relocated { from: ChangeSubject, to: ChangeSubject },
}
```

### `MemberMapping`

```rust
// report.rs:747
pub struct MemberMapping {
    pub old_name: String,
    pub new_name: String,
}
```

### `MigrationTarget`

A structural migration target detected by same-directory member overlap.

```rust
// report.rs:759
pub struct MigrationTarget {
    pub removed_symbol: String,
    pub removed_qualified_name: String,
    pub removed_package: Option<String>,
    pub replacement_symbol: String,
    pub replacement_qualified_name: String,
    pub replacement_package: Option<String>,
    pub matching_members: Vec<MemberMapping>,
    pub removed_only_members: Vec<String>,
    pub overlap_ratio: f64,
    pub old_extends: Option<String>,
    pub new_extends: Option<String>,
}
```

### `ImpactAnalysis`

```rust
// report.rs:793
pub struct ImpactAnalysis {
    pub internal_dependents: Vec<Dependent>,
    pub transitive_dependents: Vec<Dependent>,
}
```

### `Dependent`

```rust
// report.rs:805
pub struct Dependent {
    pub file: PathBuf,
    pub line: usize,
    pub symbol: String,
}
```

### `ManifestChange<L>`

A breaking change in a package manifest.

```rust
// report.rs:814
pub struct ManifestChange<L: Language> {
    pub field: String,
    pub change_type: L::ManifestChangeType,
    pub before: Option<String>,
    pub after: Option<String>,
    pub description: String,
    pub is_breaking: bool,
    pub source_package: Option<String>,
}
```

### `AnalysisMetadata`

```rust
// report.rs:843
pub struct AnalysisMetadata {
    pub call_graph_analysis: String,
    pub tool_version: String,
    pub llm_usage: Option<LlmUsage>,
}
```

### `LlmUsage`

```rust
// report.rs:857
pub struct LlmUsage {
    pub total_calls: usize,
    pub spec_inference_calls: usize,
    pub comparison_calls: usize,
    pub propagation_calls: usize,
    pub total_input_tokens: usize,
    pub total_output_tokens: usize,
    pub estimated_cost_usd: f64,
    pub circuit_breaker_triggered: bool,
}
```

### `InferredRenamePatterns`

Rename patterns discovered by the LLM rename inference phase.

```rust
// report.rs:873
pub struct InferredRenamePatterns {
    pub constant_patterns: Vec<InferredConstantPattern>,
    pub interface_mappings: Vec<InferredInterfaceMapping>,
    pub metadata: InferenceMetadata,
}
```

### `InferredConstantPattern`

```rust
// report.rs:890
pub struct InferredConstantPattern {
    pub match_regex: String,
    pub replace: String,
    pub hit_count: usize,
    pub total_removed: usize,
}
```

### `InferredInterfaceMapping`

```rust
// report.rs:902
pub struct InferredInterfaceMapping {
    pub old_name: String,
    pub new_name: String,
    pub confidence: String,          // "high", "medium", or "low"
    pub reason: String,
    pub member_overlap_ratio: f64,
}
```

### `LlmApiChange`

An API change detected by LLM file-level analysis.

```rust
// report.rs:918
pub struct LlmApiChange {
    pub file_path: String,
    pub symbol: String,
    pub change: String,
    pub description: String,
    pub removal_disposition: Option<RemovalDisposition>,
}
```

### `InferenceMetadata`

```rust
// report.rs:929
pub struct InferenceMetadata {
    pub llm_calls: usize,
    pub constant_hit_rate: f64,
    pub interface_mappings_found: usize,
}
```

### `AnalysisResult<L>`

Results from the full analysis pipeline. Produced by the orchestrator,
consumed by `Language::build_report()`. Not serialized to JSON output.

```rust
// report.rs:948
pub struct AnalysisResult<L: Language> {
    pub structural_changes: Arc<Vec<StructuralChange>>,
    pub behavioral_changes: Vec<BehavioralChange<L>>,
    pub manifest_changes: Vec<ManifestChange<L>>,
    pub llm_api_changes: Vec<LlmApiChange>,
    pub old_surface: Arc<ApiSurface<L::SymbolData>>,
    pub new_surface: Arc<ApiSurface<L::SymbolData>>,
    pub inferred_rename_patterns: Option<InferredRenamePatterns>,
    pub container_changes: Vec<(String, Vec<ContainerChange>)>,
    pub extensions: L::AnalysisExtensions,
    pub degradation: Arc<DegradationTracker>,
}
```

---

## Module: `bu` (`crates/core/src/types/bu.rs`)

### `ChangedFunction`

A function whose body changed between two git refs. Produced by
`Language::parse_changed_functions()`.

```rust
// bu.rs:26
pub struct ChangedFunction {
    pub qualified_name: String,
    pub name: String,
    pub file: PathBuf,
    pub line: usize,                     // Line number in NEW version (1-indexed)
    pub kind: SymbolKind,
    pub visibility: Visibility,
    pub old_body: Option<String>,        // None if function was added -- NOT bare String
    pub new_body: Option<String>,        // None if function was removed
    pub old_signature: Option<String>,   // None if function was added
    pub new_signature: Option<String>,   // None if function was removed
}
```

### `TestDiff`

Diff of a test file between two refs. Uses text-based assertion detection.

```rust
// bu.rs:71
pub struct TestDiff {
    pub test_file: PathBuf,
    pub removed_assertions: Vec<String>,
    pub added_assertions: Vec<String>,
    pub has_assertion_changes: bool,
    pub full_diff: String,
}
```

### `FunctionSpec`

Inferred behavioral specification for a function. Template-constrained LLM
output.

```rust
// bu.rs:102
pub struct FunctionSpec {
    pub preconditions: Vec<Precondition>,
    pub postconditions: Vec<Postcondition>,
    pub error_behavior: Vec<ErrorBehavior>,
    pub side_effects: Vec<SideEffect>,
    pub notes: Vec<String>,
}
```

### `Precondition`

```rust
// bu.rs:127
pub struct Precondition {
    pub parameter: String,
    pub condition: String,
    pub on_violation: String,
}
```

### `Postcondition`

```rust
// bu.rs:140
pub struct Postcondition {
    pub condition: String,
    pub returns: String,
}
```

### `ErrorBehavior`

```rust
// bu.rs:150
pub struct ErrorBehavior {
    pub trigger: String,
    pub error_type: String,
    pub message_pattern: Option<String>,
}
```

### `SideEffect`

```rust
// bu.rs:163
pub struct SideEffect {
    pub target: String,
    pub action: String,
    pub condition: Option<String>,
}
```

### `EvidenceType`

How a behavioral change was detected. 4 variants.

```rust
// bu.rs:180
pub enum EvidenceType {
    TestDelta,
    LlmAnalysis,
    BodyAnalysis,
    CallGraphPropagation,
}
```

Serde: `#[serde(rename_all = "snake_case")]`.

### `BehavioralBreak<L>`

A detected behavioral breaking change. Produced by the BU pipeline.

```rust
// bu.rs:215
pub struct BehavioralBreak<L: Language> {
    pub symbol: String,                      // Affected PUBLIC symbol
    pub caused_by: String,                   // Function that actually changed
    pub call_path: Vec<String>,              // Call path from symbol to caused_by
    pub evidence_description: String,
    pub confidence: f64,                     // Bare f64 (unlike BehavioralChange)
    pub description: String,
    pub category: Option<L::Category>,
    pub evidence_type: EvidenceType,         // Bare EvidenceType (unlike BehavioralChange)
    pub is_internal_only: Option<bool>,
}
```

### `BodyAnalysisResult`

A single result from deterministic body analysis. Not serializable (no Serialize/Deserialize).

```rust
// bu.rs:263
pub struct BodyAnalysisResult {
    pub description: String,
    pub category_label: Option<String>,
    pub confidence: f64,
}
```

### `Caller`

A function that calls another (for call graph walking). Not serializable.

```rust
// bu.rs:273
pub struct Caller {
    pub qualified_name: String,
    pub file: PathBuf,
    pub line: usize,
    pub visibility: Visibility,
    pub body: String,
    pub signature: String,
}
```

### `Reference`

A reference to a symbol found by cross-file search. Not serializable.

```rust
// bu.rs:296
pub struct Reference {
    pub file: PathBuf,
    pub line: usize,
    pub local_binding: String,
    pub enclosing_symbol: Option<String>,
}
```

### `TestFile`

A test file associated with a source file. Not serializable.

```rust
// bu.rs:311
pub struct TestFile {
    pub path: PathBuf,
    pub convention: TestConvention,
}
```

### `TestConvention`

How a test file is associated with its source file. Not serializable.

```rust
// bu.rs:322
pub enum TestConvention {
    DotTest,                // e.g., `foo.test.ts`
    DotSpec,                // e.g., `foo.spec.ts`
    TestsDir,               // e.g., `__tests__/foo.ts`
    Suffix(String),         // e.g., Go `_test.go`
    MirrorTree(String),     // e.g., Java `src/test/java/...`
}
```

### `BreakingVerdict`

Verdict from spec comparison.

```rust
// bu.rs:342
pub struct BreakingVerdict {
    pub is_breaking: bool,
    pub reasons: Vec<String>,
    pub confidence: f64,
}
```

---

## Module: `change_subject` (`crates/core/src/types/change_subject.rs`)

### `ChangeSubject`

What aspect of a symbol was affected by a change. Internally tagged with
`#[serde(tag = "type", rename_all = "snake_case")]`. 10 variants.

```rust
// change_subject.rs:22
pub enum ChangeSubject {
    Symbol { kind: SymbolKind },
    Member { name: String, kind: SymbolKind },
    Parameter { name: String },
    ReturnType,
    Visibility,
    Modifier { modifier: String },
    TypeParameter { name: String },
    BaseClass,
    InterfaceImpl { interface_name: String },
    UnionValue { value: String },
}
```

---

## Module: `envelope` (`crates/core/src/types/envelope.rs`)

### `ReportEnvelope`

Self-describing container for an analysis report. Language-agnostic fields
are always accessible; `language_report` requires knowing the concrete
`Language` to deserialize.

```rust
// envelope.rs:21
pub struct ReportEnvelope {
    pub language: String,                              // Matches L::NAME
    pub version: String,                               // Tool version
    pub summary: AnalysisSummary,
    pub structural_changes: Vec<StructuralChange>,
    pub language_report: serde_json::Value,             // Call .language_report::<L>() to deserialize
}
```

### `AnalysisSummary`

Aggregate statistics readable without language knowledge.

```rust
// envelope.rs:59
pub struct AnalysisSummary {
    pub total_structural_breaking: usize,
    pub total_structural_non_breaking: usize,
    pub total_behavioral_changes: usize,
    pub total_manifest_changes: usize,
    pub packages_analyzed: usize,
    pub files_changed: usize,
    pub by_change_type: ChangeTypeCounts,
}
```

### `ChangeTypeCounts`

Breakdown of structural changes by lifecycle type.

```rust
// envelope.rs:78
pub struct ChangeTypeCounts {
    pub added: usize,
    pub removed: usize,
    pub changed: usize,
    pub renamed: usize,
    pub relocated: usize,
}
```

### `LanguageReport<L>`

Language-specific section of the report.

```rust
// envelope.rs:91
pub struct LanguageReport<L: Language> {
    pub behavioral_changes: Vec<LanguageBehavioralChange<L>>,
    pub manifest_changes: Vec<LanguageManifestChange<L>>,
    pub data: L::ReportData,
}
```

### `LanguageBehavioralChange<L>`

A behavioral change with language-specific types. Used in the envelope layer.

```rust
// envelope.rs:105
pub struct LanguageBehavioralChange<L: Language> {
    pub symbol: String,
    pub category: Option<L::Category>,
    pub description: String,
    pub confidence: f64,              // Bare f64 (unlike report::BehavioralChange)
    pub evidence: L::Evidence,        // Language-typed evidence
    pub is_internal_only: bool,       // Bare bool (unlike report::BehavioralChange)
}
```

### `LanguageManifestChange<L>`

A manifest change with language-specific types. Used in the envelope layer.

```rust
// envelope.rs:117
pub struct LanguageManifestChange<L: Language> {
    pub field: String,
    pub change_type: L::ManifestChangeType,
    pub before: Option<String>,
    pub after: Option<String>,
    pub description: String,
    pub is_breaking: bool,
}
```

---

## Re-exports (`crates/core/src/types/mod.rs`)

```rust
pub use bu::*;
pub use change_subject::*;
pub use envelope::*;
pub use report::*;
pub use surface::*;
```

All public types from all submodules are re-exported from `crates::core::types`.
