# Semver Analyzer

Deterministic semantic versioning breaking-change detection for TypeScript/React
and Java. Compares two git refs, detects breaking changes, generates Konveyor
migration rules.

## Quick Reference

```sh
cargo fmt --all -- --check
cargo clippy --workspace -- -D warnings
cargo test --workspace
cargo build --workspace

# With Java support
cargo test --workspace --features java
```

## Architecture

```
src/                  CLI binary, orchestrator (Analyzer<L>), progress reporting
crates/core/          Language-agnostic: types, traits, diff engine, rename detection
crates/ts/            TypeScript/React: OXC extraction, source profiles, composition trees
crates/java/          Java: tree-sitter extraction, Maven/Gradle, module-info (feature-gated)
crates/llm/           LLM behavioral analysis (goose CLI, language-agnostic)
crates/konveyor-core/ Shared Konveyor rule types, fix strategies, consolidation
```

## Code Placement Rules (CRITICAL)

| Code type | Correct crate |
|-----------|--------------|
| Traits, types, diff algorithm, rename/relocation/migration detection | `crates/core/` |
| TypeScript extraction, JSX/CSS analysis, source profiles, composition trees | `crates/ts/` |
| Java extraction, SD pipeline, annotations, module system | `crates/java/` |
| LLM prompts, response parsing, spec comparison | `crates/llm/` |
| Konveyor rule types, fix strategies, consolidation | `crates/konveyor-core/` |
| CLI, orchestrator, progress, error rendering | `src/` |

### Rules

1. **`crates/core/` must NEVER import from `crates/ts/`, `crates/java/`, or
   `crates/llm/`.** Core defines contracts; language crates implement them.
   If you need language-specific behavior in core, add a trait method with a
   default impl.

2. **`crates/llm/` must NEVER import from language crates.** Language-specific
   data flows via parameters (`LlmCategoryDefinition` defined in core).

3. **Per-symbol metadata goes in `Language::SymbolData`.** TypeScript has
   `TsSymbolData`; Java has `JavaSymbolData`. Never add language-specific
   fields to `Symbol` directly.

4. **Pipeline extension data goes in `Language::AnalysisExtensions`.**
   TypeScript has `TsAnalysisExtensions`; Java has `JavaAnalysisExtensions`.
   The orchestrator never downcasts.

5. **Clippy must be clean.** `cargo clippy --workspace --all-targets` — zero
   warnings, zero errors.

## Three Pipelines

- **TD (Top-Down)** — Always runs. Structural API diff from `.d.ts`/`.java`
  surfaces. Key code: `crates/core/src/diff/`
- **SD (Source-Level)** — Default. Deterministic AST-based analysis of source
  changes (DOM, CSS, ARIA, composition trees). Key code:
  `crates/ts/src/sd_pipeline.rs`, `crates/java/src/sd_pipeline.rs`
- **BU (Bottom-Up)** — Opt-in via `--behavioral`. LLM behavioral analysis.
  Key code: `crates/llm/src/`, `src/orchestrator.rs`

## Key Invariants

### EdgeStrength (4 variants)

`Allowed` (CHP=NO, PMC=NO), `Structural` (CHP=YES, PMC=NO),
`Wrapper` (CHP=NO, PMC=YES), `Required` (CHP=YES, PMC=YES).
CHP = child-must-have-parent, PMC = parent-must-have-child. These drive
conformance rule generation: `Structural` generates `notParent` only,
`Wrapper` generates `requiresChild` only, `Required` generates both,
`Allowed` generates neither.

### Rename Detection

4-pass fingerprint algorithm in `crates/core/src/diff/rename.rs`. Thresholds:
same-family 0.15, cross-family 0.50, ambiguous 0.45. Cross-family renames
require a sibling rename between the same directories. Before modifying,
read `design/rename-detector-verification.md` (15 true, 28 false renames).

### collapse_internal_nodes

Processes ONE internal node at a time, preferring leaves. Never process all
at once — breaks multi-level chains. Collapsed edges inherit the stronger
strength and propagate BEM evidence.

### BEM Block Independence

Components with their own BEM block are independent, not children. Enforced
in `classify_bem_relationship()` and `infer_ownership_by_name_prefix()`.
Never add composition edges between components with different BEM blocks.

### Type-Incompatible Member Renames

When a property is renamed AND its type changes structurally, emit a single
`Changed` entry, NOT separate Removed + Added. This preserves the old-to-new
linkage for fix strategies.

## Deep Dive References

For comprehensive rules on composition trees, conformance rule generation,
deprecated replacement detection, Konveyor rule precision, error handling,
and PatternFly-specific ground truth data, see
[`design/agent-guide.md`](design/agent-guide.md).

### For AI agents (`design/`)

| File | Purpose |
|------|---------|
| [`design/agent-guide.md`](design/agent-guide.md) | Full agent guide (composition trees, conformance rules, error handling) |
| [`design/architecture.md`](design/architecture.md) | Crate dependency graph, trait system, data flow |
| [`design/pipelines.md`](design/pipelines.md) | TD/SD/BU pipeline deep dive |
| [`design/core-crate.md`](design/core-crate.md) | Core crate: types, diff engine, rename detection |
| [`design/ts-crate.md`](design/ts-crate.md) | TypeScript crate: extraction, source profiles, composition trees |
| [`design/java-crate.md`](design/java-crate.md) | Java crate: extraction, SD pipeline, module system |
| [`design/llm-crate.md`](design/llm-crate.md) | LLM crate: prompts, spec comparison, CLI contract |
| [`design/data-types.md`](design/data-types.md) | Every public struct/enum with fields and variants |
| [`design/testing.md`](design/testing.md) | Test infrastructure, fixtures, snapshot patterns |
| [`design/adding-features.md`](design/adding-features.md) | Step-by-step guides for common modifications |
| [`design/rename-detector-verification.md`](design/rename-detector-verification.md) | Rename detection ground truth (15 true, 28 false) |
| [`design/composition-ground-truth.md`](design/composition-ground-truth.md) | Composition tree & edge verification data (78 edges) |

### For humans (`docs/`)

| File | Purpose |
|------|---------|
| [`docs/onboarding.md`](docs/onboarding.md) | Mental map of the project for new engineers |
| [`docs/typescript-guide.md`](docs/typescript-guide.md) | What the analyzer detects for TypeScript |
| [`docs/konveyor-rules.md`](docs/konveyor-rules.md) | Konveyor rule generation reference |
| [`docs/report-format.md`](docs/report-format.md) | JSON report schema |
| [`docs/patternfly-walkthrough.md`](docs/patternfly-walkthrough.md) | Step-by-step PatternFly analysis |
| [`docs/llm-integration.md`](docs/llm-integration.md) | LLM setup and usage |
| [`open-issues.md`](open-issues.md) | Known bugs with root cause analysis |
