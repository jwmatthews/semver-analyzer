# Architecture

## Crate Dependency Graph

```
                    src/ (binary)
                   /    |        \
                  /     |         \
           crates/ts  crates/java  crates/llm
              |    \      |       /
              |     \     |      /
              |   crates/konveyor-core
              |        |
              +--------+
                   |
              crates/core
```

**Direction**: arrows point from dependent to dependency. `core` is at the bottom -- it knows nothing about any language crate.

### What Lives Where

| Crate | Responsibility | Key Invariant |
|-------|---------------|---------------|
| `crates/core` | Language-agnostic types, traits, structural diff engine, rename detection, relocation detection, migration detection | **Never imports from ts/, java/, llm/** |
| `crates/ts` | TypeScript/React analysis: OXC-based `.d.ts` extraction, source profile extraction from `.tsx`, composition tree builder, JSX/CSS diffing, Konveyor rule generation | Implements `Language` for `TypeScript` |
| `crates/java` | Java analysis: tree-sitter extraction, Maven/Gradle manifest diffing, module-info analysis, cross-file index | Implements `Language` for `Java` |
| `crates/llm` | LLM behavioral analysis: shells out to external CLI (goose), prompt construction, JSON response parsing | Implements `BehaviorAnalyzer` trait. Language-agnostic. |
| `crates/konveyor-core` | Shared Konveyor rule types, fix strategies, rule consolidation/deduplication logic | Re-exports types from external `konveyor-core` crate, adds project-specific helpers |
| `src/` | CLI binary: argument parsing, command dispatch, orchestrator (`Analyzer<L>`), progress reporting, error rendering | Glues everything together via generic `L: Language` |

### The `Language` Trait (Central Abstraction)

Defined in `crates/core/src/traits.rs`. This is the single integration point that language crates implement.

```rust
pub trait Language: LanguageSemantics<Self::SymbolData> + MessageFormatter + Send + Sync + 'static {
    type SymbolData;           // Per-symbol metadata (e.g., TsSymbolData, JavaSymbolData)
    type Category;             // Behavioral change categories
    type ManifestChangeType;   // Manifest change types
    type Evidence;             // Evidence for behavioral changes
    type ReportData;           // Language-specific report data
    type AnalysisExtensions;   // Extended analysis results (SD pipeline output)

    const NAME: &'static str;
    const MANIFEST_FILES: &'static [&'static str];
    const SOURCE_FILE_PATTERNS: &'static [&'static str];
    const RENAMEABLE_SYMBOL_KINDS: &'static [SymbolKind];

    // ~20 methods: extract, diff, build_report, run_extended_analysis, etc.
}
```

**`LanguageSemantics<M>`** (16 methods with defaults): Language-specific semantic rules consumed by the diff engine. Key methods:
- `is_member_addition_breaking()` -- TS: only if required member; Java: only if abstract method on interface
- `same_family()` -- TS: same canonical component directory; Java: same package
- `visibility_rank()` -- numeric ordering for visibility comparison
- `parse_union_values()` -- fine-grained union literal diffing (TS-specific)

**`HierarchySemantics<M>`**: Component hierarchy inference (React-specific). Methods for cross-family relationship detection, deterministic hierarchy computation.

**`BehaviorAnalyzer`**: LLM-based behavioral analysis (language-agnostic). Methods: `infer_spec`, `specs_are_breaking`, `check_propagation`.

## Data Flow

```
Git repo + two refs (from, to)
        |
        v
  +-----------+     +-----------+     +-------------+
  | Worktree  | --> | Extract   | --> | ApiSurface  |
  | (tsc/mvn) |     | (.d.ts/   |     | (symbols,   |
  |           |     |  .java)   |     |  members)   |
  +-----------+     +-----------+     +------+------+
                                             |
        +------------------------------------+
        |                                    |
        v                                    v
  +------------+                     +--------------+
  | TD Pipeline|                     | SD Pipeline  |
  | (core diff |                     | (source-level|
  |  engine)   |                     |  AST diff)   |
  +-----+------+                     +------+-------+
        |                                   |
        v                                   v
  StructuralChange[]              SdPipelineResult
        |                                   |
        +-------------------+---------------+
                            |
                            v
                   AnalysisReport<L>
                            |
                            v
                   KonveyorRule[] + FixStrategy[]
                            |
                            v
                   ruleset.yaml / rules.yaml / fix-strategies.json
```

## Orchestrator (`src/orchestrator.rs`)

The `Analyzer<L: Language>` struct holds three `Arc<L>` instances: one default, one with from-ref build config, one with to-ref build config.

### Two Pipeline Modes

**`run_v2()` (default, SD pipeline)**:
1. Extract both surfaces in parallel via `tokio::join!`
2. After extraction: structural diff + manifest diff
3. SD analysis runs concurrently (worktree paths passed via `mpsc::channel`)
4. Merge results into `AnalysisResult<L>`

**`run()` (BU pipeline, `--behavioral`)**:
1. TD and BU run concurrently via `tokio::join!`
2. TD: extract, diff, broadcast structural breaks
3. BU Phase 1: parse git diff, test analysis
4. BU Phase 2: LLM file analysis (5 concurrent tasks)
5. Cross-pipeline coordination via `SharedFindings<L>` (DashMap + broadcast channel)

## Error Handling

Three severity tiers:
- **Fatal**: No git repo, invalid refs, tsc fails -- abort with user-facing tip via `ErrorTip` trait
- **Degraded**: LLM timeout, parse failure -- skip + log via `DegradationTracker`, continue
- **Recoverable**: Single file failure -- skip symbol, continue

The `ErrorTip` trait + `Diagnosed` wrapper pattern: language crates define error enums implementing `ErrorTip`, call `.diagnose()` at boundaries, CLI extracts tips via `downcast_ref::<DiagnosedError>()`.

## Configuration

All configuration is via CLI arguments (no config files). Key flags:
- `--behavioral`: Use BU pipeline with LLM instead of default SD pipeline
- `--no-llm`: Disable LLM even in BU pipeline
- `--llm-command`: Custom LLM CLI command (default: goose)
- `--pipeline-v2`: Explicitly select SD pipeline (now the default)
- `--from-build-command` / `--to-build-command`: Custom build commands per ref
- `--rename-patterns`: Path to YAML file with custom rename/composition rules
