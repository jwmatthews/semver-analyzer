# Semver Analyzer — Agent Guide

## Project Overview

A multi-language semver analysis tool built in Rust. Compares two versions of a
library (via git refs), detects breaking changes, and generates Konveyor
migration rules with fix strategies.

### Architecture

- `crates/core/` — Language-agnostic diff engine, types, traits
- `crates/ts/` — TypeScript/React-specific: source profiles, JSX analysis, CSS
  profiles, konveyor rule generation
- `crates/konveyor-core/` — Konveyor rule types and fix strategy framework
- `crates/llm/` — LLM integration for behavioral analysis
- `src/orchestrator.rs` — Pipeline orchestrator (TD+BU or TD+SD)
- `src/main.rs` — CLI entry point

### Three Pipelines

The analyzer has three pipelines. The `--pipeline-v2` flag controls which
combination runs.

#### TD (Top-Down) — Structural API Diff

**Always runs.** Extracts `.d.ts` API surfaces at both git refs, then diffs
them to detect:

- Renamed, removed, added symbols (constants, interfaces, type-aliases)
- Type changes on properties
- Signature changes (base class, return type)
- Relocations (moved to deprecated/, next/ promoted)
- Member-level renames within interfaces

Key files: `crates/core/src/diff/` (mod.rs, rename.rs, compare.rs, relocate.rs,
migration.rs)

#### BU (Bottom-Up) — Behavioral Analysis (v1 only)

**Runs when `--pipeline-v2` is NOT set.** Walks the git diff bottom-up:

1. Parse changed functions from git diff
2. For each changed function, find associated test files
3. Check test assertion changes → behavioral break
4. If private function has behavioral break → walk UP call graph to public API
5. Optionally runs LLM analysis on changed files for deeper behavioral insights

Key files: `src/orchestrator.rs` (run() method, lines 56–305)

#### SD (Source-Level Diff) — Deterministic Source Analysis (v2 only)

**Runs when `--pipeline-v2` IS set.** Replaces BU with deterministic AST-based
analysis:

1. Extract `ComponentSourceProfile` for each component at both refs
2. Diff profiles to produce `SourceLevelChange` entries:
   - DOM structure, ARIA, role, data attribute changes
   - CSS token usage, prop-style bindings
   - Portal usage, context dependencies
   - Forward ref, memo, composition
   - Prop defaults, children slot path
   - Managed attribute overrides (prop-overrides-attribute)
3. Build composition trees and conformance checks
4. Extract CSS profiles for class/variable removal detection

Key files: `crates/ts/src/source_profile/`, `crates/ts/src/sd_pipeline.rs`,
`crates/ts/src/composition/`

### Pipeline Selection

```sh
# v1: TD + BU (structural + behavioral)
semver-analyzer analyze typescript --repo ... --from v5 --to v6

# v2: TD + SD (structural + source-level) — default for pipeline runs
semver-analyzer analyze typescript --repo ... --from v5 --to v6 --pipeline-v2
```

Both produce an `AnalysisReport` with the same top-level structure. v1 populates
`breaking_behavioral_changes`, v2 populates `sd_result` (source_level_changes,
composition_trees, conformance_checks, etc.).

Rule generation (`konveyor` subcommand) also accepts `--pipeline-v2` to enable
v2-specific rules (composition, conformance, prop-to-child migration, test
impact, CSS removal, prop-attribute-override).

## Key Rules for Agents

### Rename Detection (CRITICAL)

**Before modifying `crates/core/src/diff/rename.rs`**, read:

- `design/rename-detector-verification.md` — Contains the verification dataset
  (15 known-true renames, 28 known-false renames with similarity scores and root
  causes), the verification procedure, and threshold boundaries.
- Run the verification procedure after any change to confirm no regressions.

### Source Profile Extraction

Source profiles are extracted in `crates/ts/src/source_profile/`. Submodules:

- `mod.rs` — Main extraction, JSX walking
- `prop_defaults.rs` — Default value extraction from destructuring
- `prop_style.rs` — Prop-to-CSS-class binding detection
- `managed_attrs.rs` — Prop-overrides-attribute dataflow tracing
- `diff.rs` — Profile diffing to produce SourceLevelChange entries
- `bem.rs` — BEM CSS structure parsing
- `children_slot.rs` — Children wrapper path tracing
- `react_api.rs` — React API usage detection (portal, memo, forwardRef)

### Konveyor Rules

- `crates/ts/src/konveyor.rs` — v1 rule generation (TD pipeline)
- `crates/ts/src/konveyor_v2.rs` — v2 rule generation (SD pipeline: composition,
  conformance, context, prop-to-child migration, test impact, CSS removal,
  prop-attribute-override)
- `crates/konveyor-core/src/lib.rs` — Shared rule types, fix strategies

### Testing

```sh
cargo test -p semver-analyzer-ts --lib    # ~589 unit tests
cargo test -p semver-analyzer-ts          # + integration tests
cargo test                                # full suite
```

## PatternFly v5 → v6 Reference

The primary test case is PatternFly React v5.4.0 → v6.4.1. Key stats:

- 15,525 total breaking changes
- 340 non-token removals, 4,094 renames (3,995 CSS tokens), 3,866 type changes
- 28 known false-positive renames (see design doc for full details)
- Full change landscape and verification data in
  `design/rename-detector-verification.md`
