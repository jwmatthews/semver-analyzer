# Language Implementation Guide

How to implement the `Language` trait for a new language. Uses TypeScript as the
reference implementation and Java for contrast.

**Source files:**
- `crates/core/src/traits.rs` -- trait definitions
- `crates/ts/src/language.rs` -- TypeScript implementation
- `crates/java/src/language.rs` -- Java implementation

---

## 1. The Language Struct

The struct is NOT a unit struct. It carries configuration needed for extraction.

### TypeScript (reference implementation)

```rust
// crates/ts/src/language.rs:31-59

#[derive(Debug, Clone)]
pub struct TypeScript {
    ref_config: RefBuildConfig,
}

impl TypeScript {
    pub fn new(build_command: Option<String>) -> Self {
        Self {
            ref_config: RefBuildConfig {
                build_command,
                ..Default::default()
            },
        }
    }

    pub fn with_ref_config(config: RefBuildConfig) -> Self {
        Self { ref_config: config }
    }
}

impl Default for TypeScript {
    fn default() -> Self {
        Self {
            ref_config: RefBuildConfig {
                build_command: Some("yarn build".to_string()),
                ..Default::default()
            },
        }
    }
}
```

### Java (contrast)

```rust
// crates/java/src/language.rs:29-107

pub struct Java {
    ref_config: Option<crate::worktree::JavaRefBuildConfig>,
    index: std::sync::Mutex<Option<crate::index::JavaIndex>>,
    repo_root: std::sync::Mutex<Option<std::path::PathBuf>>,
}

impl Java {
    pub fn new() -> Self { /* all fields None/default */ }
    pub fn with_ref_config(config: JavaRefBuildConfig) -> Self { /* stores config */ }
}

impl Default for Java {
    fn default() -> Self { Self::new() }
}
```

Java carries extra state (`index`, `repo_root`) for its lazily-built cross-file
index used by `find_callers`/`find_references`.

---

## 2. The Six Associated Types

The `Language` trait requires exactly six associated types. All six must satisfy
the trait bounds shown in column 3.

| # | Associated Type | Bounds | TypeScript concrete type | Java concrete type |
|---|----------------|--------|------------------------|--------------------|
| 1 | `SymbolData` | `Debug + Clone + Default + PartialEq + Eq + Serialize + DeserializeOwned + Send + Sync` | `TsSymbolData` | `JavaSymbolData` |
| 2 | `Category` | `Debug + Clone + Serialize + DeserializeOwned + Eq + Hash + Send + Sync` | `TsCategory` | `JavaCategory` |
| 3 | `ManifestChangeType` | `Debug + Clone + Serialize + DeserializeOwned + Eq + PartialEq + Send + Sync` | `TsManifestChangeType` | `JavaManifestChangeType` |
| 4 | `Evidence` | `Debug + Clone + Serialize + DeserializeOwned + Send + Sync` | `TsEvidence` | `JavaEvidence` |
| 5 | `ReportData` | `Debug + Clone + Serialize + DeserializeOwned + Send + Sync` | `TsReportData` | `JavaReportData` |
| 6 | `AnalysisExtensions` | `Debug + Clone + Default + Serialize + DeserializeOwned + Send + Sync` | `TsAnalysisExtensions` | `JavaAnalysisExtensions` |

Source: `crates/core/src/traits.rs:611-653`

### 2.1 SymbolData -- per-symbol metadata

Stored in `Symbol<M>.language_data`. Languages without per-symbol metadata use `()`.

```rust
// crates/ts/src/symbol_data.rs:15-31

#[derive(Debug, Clone, Default, PartialEq, Eq, Serialize, Deserialize)]
pub struct TsSymbolData {
    /// Components this component renders internally (JSX tree).
    /// Used for hierarchy inference.
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub rendered_components: Vec<String>,

    /// CSS class tokens (e.g., `["inputGroup", "inputGroupItem"]`).
    /// Extracted from `styles.xxx` references.
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub css: Vec<String>,
}
```

### 2.2 Category -- behavioral change categories

```rust
// crates/ts/src/language.rs:65-84

#[derive(Debug, Clone, PartialEq, Eq, PartialOrd, Ord, Hash, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum TsCategory {
    DomStructure,
    CssClass,
    CssVariable,
    Accessibility,
    DefaultValue,
    LogicChange,
    DataAttribute,
    RenderOutput,
}
```

### 2.3 ManifestChangeType -- package manifest changes

```rust
// crates/ts/src/language.rs:87-100

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum TsManifestChangeType {
    EntryPointChanged,
    ExportsEntryRemoved,
    ExportsEntryAdded,
    ExportsConditionRemoved,
    ModuleSystemChanged,
    PeerDependencyAdded,
    PeerDependencyRemoved,
    PeerDependencyRangeChanged,
    EngineConstraintChanged,
    BinEntryRemoved,
}
```

### 2.4 Evidence -- evidence data on behavioral changes

```rust
// crates/ts/src/language.rs:103-124

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(tag = "type", rename_all = "snake_case")]
pub enum TsEvidence {
    TestDelta {
        removed_assertions: Vec<String>,
        added_assertions: Vec<String>,
    },
    JsxDiff {
        element_before: Option<String>,
        element_after: Option<String>,
        change_description: String,
    },
    CssScan {
        change_description: String,
    },
    LlmAnalysis {
        has_test_context: bool,
        spec_summary: String,
    },
}
```

### 2.5 ReportData -- per-TypeSummary report data

Flattened into the parent `TypeSummary` JSON via `#[serde(flatten)]`.

```rust
// crates/ts/src/language.rs:134-145

#[derive(Debug, Clone, Default, Serialize, Deserialize)]
pub struct TsReportData {
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub child_components: Vec<ChildComponent>,

    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub expected_children: Vec<ExpectedChild>,
}
```

- `child_components` -- discovered child/sibling components with absorbed members
- `expected_children` -- expected direct children from hierarchy inference

`ChildComponent` (lines 148-161) has fields: `name: String`, `status: ChildComponentStatus`, `known_members: Vec<String>`, `absorbed_members: Vec<String>`.

`ExpectedChild` is defined in `crates/core/` (shared across languages).

### 2.6 AnalysisExtensions -- pipeline-level extensions

```rust
// crates/ts/src/extensions.rs:22-40

#[derive(Debug, Clone, Default, Serialize, Deserialize)]
pub struct TsAnalysisExtensions {
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub sd_result: Option<SdPipelineResult>,

    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub hierarchy_deltas: Vec<HierarchyDelta>,

    #[serde(default, skip_serializing_if = "HashMap::is_empty")]
    pub new_hierarchies: HashMap<String, HashMap<String, Vec<ExpectedChild>>>,
}
```

Languages without extended analysis use `EmptyExtensions` (a unit struct in core
that implements the required bounds).

---

## 3. The Four Constants

Source: `crates/core/src/traits.rs:656-696`

| Constant | Type | TypeScript value | Java value |
|----------|------|-----------------|------------|
| `NAME` | `&'static str` | `"typescript"` | `"java"` |
| `MANIFEST_FILES` | `&'static [&'static str]` | `&["package.json"]` | `&["pom.xml", "build.gradle", "build.gradle.kts"]` |
| `SOURCE_FILE_PATTERNS` | `&'static [&'static str]` | `&["*.ts", "*.tsx"]` | `&["*.java"]` |
| `RENAMEABLE_SYMBOL_KINDS` | `&'static [SymbolKind]` | `&[Interface, Class]` | `&[Interface, Class, Enum]` |

```rust
// crates/ts/src/language.rs:373-377

const RENAMEABLE_SYMBOL_KINDS: &'static [SymbolKind] =
    &[SymbolKind::Interface, SymbolKind::Class];
const NAME: &'static str = "typescript";
const MANIFEST_FILES: &'static [&'static str] = &["package.json"];
const SOURCE_FILE_PATTERNS: &'static [&'static str] = &["*.ts", "*.tsx"];
```

---

## 4. Required Methods on Language

These are the methods you MUST implement (no default). Source: `crates/core/src/traits.rs:700-800`.

| Method | Signature | Purpose |
|--------|-----------|---------|
| `extract` | `(&self, repo: &Path, git_ref: &str, degradation: Option<&DegradationTracker>) -> Result<ApiSurface<Self::SymbolData>>` | Extract public API surface at a git ref |
| `parse_changed_functions` | `(&self, repo: &Path, from_ref: &str, to_ref: &str) -> Result<Vec<ChangedFunction>>` | Parse git diff to find all changed function bodies |
| `find_callers` | `(&self, file: &Path, symbol_name: &str) -> Result<Vec<Caller>>` | Find callers of a function (for propagation analysis) |
| `find_references` | `(&self, file: &Path, symbol_name: &str) -> Result<Vec<Reference>>` | Find all references to a symbol across the project |
| `find_tests` | `(&self, repo: &Path, source_file: &Path) -> Result<Vec<TestFile>>` | Find test files associated with a source file |
| `diff_test_assertions` | `(&self, repo: &Path, test_file: &TestFile, from_ref: &str, to_ref: &str) -> Result<TestDiff>` | Diff test assertions between two refs |
| `diff_manifest_content` | `(old: &str, new: &str) -> Vec<ManifestChange<Self>>` | Diff manifest content (static method) |
| `should_exclude_from_analysis` | `(path: &Path) -> bool` | Whether a file should be excluded from BU analysis (static method) |
| `build_report` | `(&self, results: &AnalysisResult<Self>, repo: &Path, from_ref: &str, to_ref: &str) -> AnalysisReport<Self>` | Build the language-specific analysis report |

### Methods with default implementations (optional to override)

| Method | Default | TypeScript override? | Purpose |
|--------|---------|---------------------|---------|
| `extract_keeping_worktree` | Calls `extract()`, returns `None` worktree handle | Yes -- shares worktrees between TD/SD | Extract + keep worktree alive |
| `discover_package_manifests` | Empty vec | Yes -- discovers workspace packages | Monorepo manifest discovery |
| `behavioral_change_kind` | Always `Function` | Yes -- `Class` for React components | Map evidence type to change granularity |
| `extract_referenced_symbols` | Empty vec | Yes -- extracts PascalCase names | Extract symbol refs from descriptions |
| `display_name` | Returns as-is | Yes -- strips file prefix | Format qualified name for display |
| `llm_categories` | Empty vec | Yes -- 8 React/CSS categories | Behavioral change categories for LLM |
| `run_extended_analysis` | Returns `Default::default()` | Yes -- runs SD pipeline | Run language-specific extended analysis |
| `finalize_extensions` | No-op, returns changes unchanged | Yes -- deprecated replacement detection | Post-process after TD + extended analysis |
| `extensions_log_summary` | Empty vec | Yes -- SD pipeline summary | Log-friendly summary lines |

---

## 5. LanguageSemantics<M> Implementation

`Language` requires `LanguageSemantics<Self::SymbolData>`. This trait is defined
at `crates/core/src/traits.rs:97-311`.

### Required methods (no default)

| Method | TypeScript behavior |
|--------|-------------------|
| `is_member_addition_breaking(&self, container, member) -> bool` | Breaking only for required members on Interface/TypeAlias |
| `same_family(&self, a, b) -> bool` | Same component directory (strips `/deprecated/`, `/next/`) |
| `same_identity(&self, a, b) -> bool` | Strips `Props` suffix before comparing |
| `visibility_rank(&self, v) -> u8` | Private(0) < Internal(1) = Protected(1) < Public(2) < Exported(3) |

### Methods with defaults that TypeScript overrides

| Method | Default | TypeScript override |
|--------|---------|-------------------|
| `parse_union_values` | `None` | Parses `'primary' \| 'secondary'` string literal unions |
| `is_async_wrapper` | `false` | `true` for `Promise<...>` |
| `format_import_change` | Generic format | TypeScript import syntax |
| `should_skip_symbol` | `false` | `true` for star re-exports (`name == "*"`) |
| `member_label` | `"members"` | `"props"` (React terminology) |
| `extract_rename_fallback_key` | `None` | Extracts CSS token value from `.d.ts` type annotation |
| `canonical_name_for_relocation` | Returns unchanged | Strips `/deprecated/` and `/next/` |
| `classify_relocation` | `None` | Detects deprecated/next transitions |
| `derive_import_subpath` | Returns package name | Appends `/deprecated` or `/next` |
| `diff_language_data` | Empty vec | Not overridden (Java overrides for annotations, throws, modifiers) |
| `post_process` | No-op | Deduplicates default export changes |
| `hierarchy` | `None` | Returns `Some(self)` -- exposes `HierarchySemantics` |
| `renames` | `None` | Returns `Some(self)` -- exposes `RenameSemantics` |
| `body_analyzer` | `None` | Returns `Some(self)` -- exposes `BodyAnalysisSemantics` |
| `primitive_type_names` | 5 common types | Adds `undefined`, `never`, `any`, `unknown` |

Source: TypeScript overrides at `crates/ts/src/language.rs:175-341`.

---

## 6. MessageFormatter Implementation

Required by `Language`. One method: `fn describe(&self, change: &StructuralChange) -> String`.

Both TypeScript and Java currently return `change.description.clone()` (descriptions
are built by the diff engine). Future phases will move description construction
into this method.

Source: `crates/ts/src/language.rs:346-361`, `crates/java/src/language.rs:688-692`.

---

## 7. Optional Capability Traits

These are NOT part of `Language`. They are accessed via optional accessors on
`LanguageSemantics` (`hierarchy()`, `renames()`, `body_analyzer()`). The
orchestrator checks for their presence and conditionally runs analysis steps.

### 7.1 HierarchySemantics<M>

Source: `crates/core/src/traits.rs:332-398`

For languages with component composition models (React, Vue, etc.). TypeScript
implements this at `crates/ts/src/language.rs:810-1325`.

| Method | Required? | Purpose |
|--------|-----------|---------|
| `family_source_paths(repo, git_ref, family_name) -> Vec<String>` | Yes | Get source file paths for a component family |
| `family_name_from_symbols(symbols) -> Option<String>` | Yes | Extract family name from symbol paths |
| `cross_family_relationships(repo, git_ref) -> Vec<(String, String, String)>` | Yes | Detect cross-family imports (e.g., React context) |
| `related_family_content(repo, git_ref, family, relationships) -> Option<String>` | Yes | Read related component source for LLM context |
| `is_hierarchy_candidate(sym) -> bool` | Yes | Filter symbols for hierarchy grouping |
| `min_components_for_hierarchy() -> usize` | No (default: 2) | Minimum family size for hierarchy inference |
| `compute_deterministic_hierarchy(new_surface, changes) -> HashMap<...>` | No (default: empty) | Compute hierarchy without LLM |

### 7.2 BodyAnalysisSemantics

Source: `crates/core/src/traits.rs:454-471`

For languages with deterministic body-level analysis. TypeScript implements this
at `crates/ts/src/language.rs:1393-1436` for JSX diff and CSS variable scanning.

| Method | Purpose |
|--------|---------|
| `analyze_changed_body(old_body, new_body, func_name, file_path) -> Vec<BodyAnalysisResult>` | Detect behavioral breaks from function body changes without LLM |

### 7.3 RenameSemantics

Source: `crates/core/src/traits.rs:409-440`

For languages with LLM-based rename pattern inference. TypeScript implements this
at `crates/ts/src/language.rs:1329-1388` to prioritize directional CSS property
suffixes.

| Method | Default | Purpose |
|--------|---------|---------|
| `sample_removed_constants(removed, added) -> Vec<&str>` | First 30 | Prioritize removed constants for LLM |
| `sample_added_constants(removed, added) -> Vec<&str>` | First 30 | Prioritize added constants for LLM |
| `min_removed_for_constant_inference() -> usize` | 50 | Minimum threshold for constant rename inference |
| `min_removed_for_interface_inference() -> usize` | 2 | Minimum threshold for interface rename inference |

---

## 8. TypeScript impl Language Block (Complete)

For reference, the full TypeScript `Language` impl block wiring:

```rust
// crates/ts/src/language.rs:365-806

impl Language for TypeScript {
    type SymbolData = TsSymbolData;
    type Category = TsCategory;
    type ManifestChangeType = TsManifestChangeType;
    type Evidence = TsEvidence;
    type ReportData = TsReportData;
    type AnalysisExtensions = TsAnalysisExtensions;

    const RENAMEABLE_SYMBOL_KINDS: &'static [SymbolKind] =
        &[SymbolKind::Interface, SymbolKind::Class];
    const NAME: &'static str = "typescript";
    const MANIFEST_FILES: &'static [&'static str] = &["package.json"];
    const SOURCE_FILE_PATTERNS: &'static [&'static str] = &["*.ts", "*.tsx"];

    fn extract(&self, repo, git_ref, degradation) -> Result<ApiSurface<TsSymbolData>> {
        // Delegates to OxcExtractor::extract_at_ref
    }
    fn extract_keeping_worktree(&self, repo, git_ref, degradation) -> Result<ExtractionWithWorktree<TsSymbolData>> {
        // Creates WorktreeGuard, extracts, returns Arc<WorktreeGuard>
    }
    fn parse_changed_functions(&self, repo, from_ref, to_ref) -> Result<Vec<ChangedFunction>> {
        // Uses crate::diff_parser to compare function bodies at both refs
    }
    fn find_callers(&self, file, symbol_name) -> Result<Vec<Caller>> {
        // Uses crate::call_graph to find callers in same-file scope
    }
    fn find_references(&self, file, symbol_name) -> Result<Vec<Reference>> {
        // Uses crate::call_graph to find references in same-file scope
    }
    fn find_tests(&self, repo, source_file) -> Result<Vec<TestFile>> {
        // Uses crate::test_analyzer to discover test files
    }
    fn diff_test_assertions(&self, repo, test_file, from_ref, to_ref) -> Result<TestDiff> {
        // Uses crate::test_analyzer to diff assertions between refs
    }
    fn diff_manifest_content(old, new) -> Vec<ManifestChange<Self>> {
        // Parses JSON, delegates to crate::manifest::diff_manifests
    }
    fn should_exclude_from_analysis(path) -> bool {
        // Excludes: index.ts, .d.ts, .test., .spec., __tests__/, dist/
    }
    fn build_report(&self, results, repo, from_ref, to_ref) -> AnalysisReport<Self> {
        // Delegates to crate::report::build_report
    }
    fn behavioral_change_kind(&self, evidence_type) -> BehavioralChangeKind {
        // TestDelta -> Function, everything else -> Class (React components)
    }
    fn extract_referenced_symbols(&self, description) -> Vec<String> {
        // Extracts <ComponentName> and `ComponentName` patterns
    }
    fn display_name(&self, qualified_name) -> String {
        // "src/Modal.tsx::Modal" -> "Modal"
    }
    fn llm_categories(&self) -> Vec<LlmCategoryDefinition> {
        // 8 categories: dom_structure, css_class, css_variable, accessibility,
        // default_value, logic_change, data_attribute, render_output
    }
    fn discover_package_manifests(repo, git_ref) -> Vec<(String, String)> {
        // Uses git ls-tree to discover packages/ workspace manifests
    }
    fn run_extended_analysis(&self, params) -> Result<TsAnalysisExtensions> {
        // Runs SD pipeline, wires orchestrator-computed CSS data
    }
    fn finalize_extensions(&self, extensions, structural_changes, repo, from_ref, to_ref) -> Arc<Vec<StructuralChange>> {
        // Deprecated replacement detection, transforms structural changes
    }
    fn extensions_log_summary(&self, extensions) -> Vec<String> {
        // SD pipeline summary: change count, composition trees, conformance checks
    }
}
```

---

## 9. Checklist for Adding a New Language

### Step 1: Create the language crate

```
crates/your-lang/
  Cargo.toml          # depends on semver-analyzer-core
  src/
    lib.rs
    language.rs        # Language trait impl
    symbol_data.rs     # YourSymbolData (or use () if no per-symbol metadata)
    extensions.rs      # YourAnalysisExtensions (or use EmptyExtensions)
    extract.rs         # API surface extraction
    diff_parser.rs     # Changed function parser
    call_graph.rs      # Caller/reference finder
    test_analyzer.rs   # Test file finder and assertion differ
    manifest.rs        # Package manifest differ
    report.rs          # Report builder
```

### Step 2: Define the six associated types

1. `SymbolData` -- per-symbol metadata struct (or `()` if none needed)
2. `Category` -- enum of behavioral change categories for LLM
3. `ManifestChangeType` -- enum of manifest change types
4. `Evidence` -- enum of evidence types for behavioral changes
5. `ReportData` -- struct for per-TypeSummary report data
6. `AnalysisExtensions` -- struct for pipeline-level extensions (or `EmptyExtensions`)

### Step 3: Implement `LanguageSemantics<YourSymbolData>`

Required methods:
- `is_member_addition_breaking` -- the most fundamental language difference
- `same_family` -- how to group related symbols for migration detection
- `same_identity` -- how to recognize companion types (e.g., `Foo` + `FooOptions`)
- `visibility_rank` -- your language's visibility ordering

Override defaults as needed for: `parse_union_values`, `is_async_wrapper`,
`format_import_change`, `should_skip_symbol`, `member_label`,
`extract_rename_fallback_key`, `canonical_name_for_relocation`,
`classify_relocation`, `derive_import_subpath`, `diff_language_data`,
`post_process`, `hierarchy`, `renames`, `body_analyzer`, `primitive_type_names`.

### Step 4: Implement `MessageFormatter`

One method: `fn describe(&self, change: &StructuralChange) -> String`.

### Step 5: Implement `Language`

- Set the six associated types
- Set the four constants (`NAME`, `MANIFEST_FILES`, `SOURCE_FILE_PATTERNS`, `RENAMEABLE_SYMBOL_KINDS`)
- Implement the nine required methods: `extract`, `parse_changed_functions`,
  `find_callers`, `find_references`, `find_tests`, `diff_test_assertions`,
  `diff_manifest_content`, `should_exclude_from_analysis`, `build_report`
- Override defaults as needed for: `extract_keeping_worktree`,
  `discover_package_manifests`, `behavioral_change_kind`,
  `extract_referenced_symbols`, `display_name`, `llm_categories`,
  `run_extended_analysis`, `finalize_extensions`, `extensions_log_summary`

### Step 6: Implement optional capability traits (if applicable)

- `HierarchySemantics<YourSymbolData>` -- if your language has component hierarchies
- `BodyAnalysisSemantics` -- if your language has deterministic body-level analysis
- `RenameSemantics` -- if your language benefits from LLM-based rename inference

Wire these via `hierarchy()`, `body_analyzer()`, and `renames()` on
`LanguageSemantics`, returning `Some(self)`.

### Step 7: Add language dispatch

- Add a CLI flag or auto-detection in `src/` (orchestrator)
- Feature-gate the crate if it has heavy dependencies (like Java's `tree-sitter`)
- Add `Analyzer<YourLang>` instantiation in the CLI

### Step 8: Verify

```sh
cargo fmt --all -- --check
cargo clippy --workspace -- -D warnings
cargo test --workspace
```

---

## 10. Key Differences By Language

### What constitutes a breaking change

| Scenario | TypeScript | Java |
|----------|-----------|------|
| Add method to interface | Breaking if required | Breaking (abstract); non-breaking (default) |
| Add method to abstract class | N/A | Breaking if abstract |
| Add field to struct/class | Non-breaking | Non-breaking |
| Remove exported function | Breaking | Breaking |
| Change return type | Breaking | Breaking |
| Add optional parameter | Non-breaking | N/A (overloads) |
| Add required parameter | Breaking | Breaking |
| Class becomes `final` | N/A | Breaking |
| Class becomes `sealed` | N/A | Breaking |
| Checked exception added | N/A | Breaking |

### Visibility models

| Level | TypeScript | Java |
|-------|-----------|------|
| Private | `private` / `#` | `private` |
| Internal | not exported from module | package-private (no modifier) |
| Protected | `protected` (rank = 1, same as Internal) | `protected` (rank = 2) |
| Public | class member | `public` (rank = 3) |
| Exported | `export` (rank = 3) | public from jar (rank = 3) |

### Companion type conventions

| Language | Convention | Example |
|----------|-----------|---------|
| TypeScript | `Props` suffix | `Button` + `ButtonProps` |
| Java | qualified name equality | `UserService` |

### Manifest files

| Language | File(s) | Key concepts |
|----------|---------|-------------|
| TypeScript | `package.json` | exports map, peerDependencies, CJS/ESM, engines |
| Java | `pom.xml`, `build.gradle`, `build.gradle.kts` | groupId:artifactId, dependencies, plugins |

---

## Hypothetical: Go (not yet implemented)

A Go implementation would differ from TypeScript in key ways:

- `SymbolData = ()` -- no per-symbol metadata needed
- `is_member_addition_breaking` -- ALWAYS breaking for interfaces (all implementors must add it)
- `same_family` -- same Go package (directory)
- `same_identity` -- strips `Options`/`Config`/`Params`/`Error`/`Func` suffixes
- `visibility_rank` -- two levels only: unexported(0), exported(1)
- `parse_union_values` -- returns `None` (Go has no union types)
- `MANIFEST_FILES = &["go.mod"]`
- `SOURCE_FILE_PATTERNS = &["*.go"]`
- No `HierarchySemantics`, `BodyAnalysisSemantics`, or `RenameSemantics`
- `AnalysisExtensions = EmptyExtensions`
