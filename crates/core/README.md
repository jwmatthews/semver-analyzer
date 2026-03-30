# semver-analyzer-core

Language-agnostic foundation for the semver-analyzer workspace. Defines shared data types, trait abstractions, the structural diff engine, and concurrent shared state for the TD/BU analysis pipelines.

No language-specific logic lives here. Language crates (e.g., `semver-analyzer-ts`) depend on this crate and implement its traits.

## Key Traits

### `Language`

The primary extension point. Language crates implement this to plug into the analysis pipeline. Composes `LanguageSemantics + MessageFormatter + Send + Sync + 'static`.

```rust
pub trait Language: LanguageSemantics + MessageFormatter + Send + Sync + 'static {
    type Category;           // Behavioral change categories (e.g., DOM, CSS, a11y)
    type ManifestChangeType; // Package manifest change types
    type Evidence;           // Evidence data for behavioral changes
    type ReportData;         // Language-specific report data

    const NAME: &'static str;
    const MANIFEST_FILES: &'static [&'static str];
    const SOURCE_FILE_PATTERNS: &'static [&'static str];

    fn extract(&self, repo: &Path, git_ref: &str) -> Result<ApiSurface>;
    fn parse_changed_functions(&self, repo: &Path, from: &str, to: &str) -> Result<Vec<ChangedFunction>>;
    fn find_callers(&self, file: &Path, symbol: &str) -> Result<Vec<Caller>>;
    fn find_tests(&self, repo: &Path, source: &Path) -> Result<Vec<TestFile>>;
    fn diff_test_assertions(&self, repo: &Path, test: &TestFile, from: &str, to: &str) -> Result<TestDiff>;
    fn build_report(results: &AnalysisResult<Self>, repo: &Path, from: &str, to: &str) -> AnalysisReport<Self>;
    // ... and more
}
```

### `LanguageSemantics`

Encodes language-specific breaking-change rules consumed by the diff engine.

| Method | Purpose |
|--------|---------|
| `is_member_addition_breaking` | Whether adding a member to a container is breaking |
| `same_family` | Whether two symbols belong to the same logical group |
| `same_identity` | Whether two symbols represent the same concept at different paths |
| `visibility_rank` | Numeric visibility rank (higher = more visible) |
| `parse_union_values` | Parse union/constrained type values for fine-grained diffing |
| `post_process` | Language-specific post-processing of the change list |

### `BehaviorAnalyzer`

Language-agnostic LLM-based behavioral analysis interface.

| Method | Purpose |
|--------|---------|
| `infer_spec` | Infer a function's behavioral spec from its body |
| `infer_spec_with_test_context` | Infer a spec grounded by test assertion diffs |
| `specs_are_breaking` | Two-tier comparison (structural then LLM fallback) |
| `check_propagation` | Check if a caller propagates a behavioral break |

### Optional Capability Traits

- **`HierarchySemantics`** -- Component hierarchy inference (React, Vue, etc.)
- **`RenameSemantics`** -- LLM-based rename pattern detection
- **`BodyAnalysisSemantics`** -- Deterministic body-level behavioral change detection

## Diff Engine

The structural diff engine compares two `ApiSurface` snapshots and produces `StructuralChange` entries using language-specific semantic rules.

```rust
// With language-specific semantics
let changes = diff_surfaces_with_semantics(&old_surface, &new_surface, &ts_semantics);

// With minimal (language-agnostic) semantics
let changes = diff_surfaces(&old_surface, &new_surface);
```

The engine runs a 6-phase pipeline:
1. Relocation detection (moved to deprecated/next)
2. Rename detection (fingerprint + LCS matching)
3. Unmatched symbol collection (added/removed)
4. Matched symbol comparison (signatures, types, visibility, members)
5. Structural migration detection (interface absorption/decomposition)
6. Language-specific post-processing

## Shared State

`SharedFindings<L>` provides concurrent coordination between the TD and BU pipelines:

- **DashMap** for structural/behavioral breaks
- **Broadcast channel** for real-time TD-to-BU notifications
- **OnceCell** for API surfaces (BU blocks until TD sets them)

```rust
let shared = Arc::new(SharedFindings::<TypeScript>::new());

// TD pipeline sets surfaces and broadcasts breaking changes
shared.set_old_surface(old_surface);
shared.insert_structural_breaks(breaking_changes);

// BU pipeline checks before analyzing each function
if should_skip_for_bu(&shared, &mut receiver, &qualified_name) {
    continue; // Already covered by TD
}
```

## Core Types

### API Surface

- `ApiSurface` -- Collection of exported symbols at a git ref
- `Symbol` -- A single exported symbol (name, kind, visibility, signature, members, etc.)
- `SymbolKind` -- Function, Class, Interface, TypeAlias, Enum, Property, etc.
- `Visibility` -- Exported, Public, Protected, Internal, Private
- `Signature` -- Parameters, return type, type parameters, async flag
- `Parameter` -- Name, type, optional/default/variadic flags

### Changes

- `StructuralChange` -- Internal diff engine output with full detail
- `StructuralChangeType` -- Added, Removed, Changed, Renamed, Relocated
- `ChangeSubject` -- What aspect changed (Symbol, Member, Parameter, ReturnType, etc.)
- `BehavioralBreak<L>` -- A detected behavioral breaking change with evidence

### Reports

- `AnalysisReport<L>` -- Full language-specific analysis report
- `ReportEnvelope` -- Self-describing, language-dispatched report container
- `AnalysisResult<L>` -- Raw pipeline output (input to `build_report`)

### CLI

Shared clap argument structs: `CommonAnalyzeArgs`, `CommonExtractArgs`, `DiffArgs`, `CommonKonveyorArgs`.

## License

Apache-2.0
