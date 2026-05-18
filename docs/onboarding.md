# Onboarding Guide

A mental map for engineers joining this project.

## What is semver-analyzer?

semver-analyzer is a CLI tool that compares two versions of a library (two git refs) and tells you exactly what broke. It detects breaking changes in APIs, component structure, CSS, accessibility attributes, and more -- then generates machine-readable migration rules that tooling can use to automate upgrades.

Think of it as "diff, but for semantic versioning" -- instead of showing you line-by-line text changes, it shows you "this function's parameter type changed from `string` to `number`" or "this React component now requires a `<ModalBody>` child."

### The Problem It Solves

When a library releases a major version bump, consumers need to know:
1. What broke? (API removals, type changes, renamed components)
2. What's the replacement? (migration targets, renamed props, new composition patterns)
3. How do I fix it? (concrete fix strategies with before/after examples)

Doing this manually by reading changelogs is slow, incomplete, and error-prone. semver-analyzer automates it by analyzing the actual code.

### Who Uses It

The primary consumer is the [Konveyor](https://konveyor.io/) project, which uses the generated migration rules to help organizations automate large-scale library upgrades. The tool has been validated against PatternFly React v5 -> v6 (56,000+ symbols, 17,000+ breaking changes).

## Languages Supported

- **TypeScript/JavaScript/React** -- Full support including component hierarchy, JSX structure, CSS, ARIA accessibility
- **Java** -- Full support including annotations, module system, serialization, Maven/Gradle manifests (requires `--features java` to build)
- **Python, Go** -- Planned but not implemented

## The Three Pipelines

semver-analyzer has three analysis pipelines. Understanding which one does what is the key to navigating the codebase.

### TD Pipeline (Top-Down) -- Always Runs

**What it does**: Extracts the public API surface at both git refs and structurally diffs them.

**For TypeScript**: Creates git worktrees, runs `npm install` + `tsc --declaration` to generate `.d.ts` files, then parses them with the OXC parser. The result is a list of every public type, function, class, interface, and their members.

**For Java**: Creates git worktrees, optionally builds with Maven/Gradle, then parses `.java` source files with tree-sitter.

**What it catches**: Removed functions, renamed interfaces, changed parameter types, visibility changes, missing exports, renamed constants, relocated components (moved to deprecated/).

**Key code**: `crates/core/src/diff/` (language-agnostic engine), `crates/ts/src/extract/` (TS extraction), `crates/java/src/extract/` (Java extraction)

### SD Pipeline (Source-Level Diff) -- Default

**What it does**: Analyzes `.tsx` / `.java` source code changes at the AST level. Fully deterministic, no LLM.

**For TypeScript/React**: Extracts 25+ attributes from each component (DOM elements rendered, ARIA attributes, CSS tokens, prop defaults, portal usage, React context dependencies, BEM structure, children slot position, cloneElement patterns). Then diffs these profiles between versions and builds composition trees (parent-child component relationships).

**For Java**: Extracts class profiles (annotations, synchronized methods, thrown exceptions, serialization fields, module directives) and diffs them.

**What it catches**: DOM structure changes, ARIA accessibility changes, CSS class/variable renames, prop default value changes, new required children in component hierarchies, prop-to-child migrations, React context dependency changes.

**Key code**: `crates/ts/src/sd_pipeline.rs`, `crates/ts/src/source_profile/`, `crates/ts/src/composition/`, `crates/java/src/sd_pipeline.rs`

### BU Pipeline (Bottom-Up) -- Opt-in via `--behavioral`

**What it does**: Uses an LLM to infer behavioral specifications from function bodies and detect behavioral changes not visible from type signatures.

**What it catches**: Logic changes within function bodies, changed error behavior, removed side effects.

**Key code**: `crates/llm/src/`, `src/orchestrator.rs` (BU scheduling)

**Why it's opt-in**: LLM analysis costs money (~$2-10 per run), is non-deterministic, and slower. The SD pipeline catches most practical breaking changes without it.

## Architecture at a Glance

```
You run:  semver-analyzer analyze typescript --repo ./my-lib --from v5 --to v6

What happens:
  1. CLI parses args                    (src/cli/mod.rs)
  2. Orchestrator sets up pipelines     (src/orchestrator.rs)
  3. Git worktrees created for v5, v6   (crates/ts/src/worktree/)
  4. npm install + tsc in each          (crates/ts/src/worktree/tsc.rs)
  5. OXC parses .d.ts files             (crates/ts/src/extract/)
  6. Types canonicalized                (crates/ts/src/canon/)
  7. Structural diff runs               (crates/core/src/diff/)
     - Relocation detection
     - 4-pass rename detection
     - Symbol-by-symbol comparison
     - Migration detection
  8. SD pipeline runs in parallel       (crates/ts/src/sd_pipeline.rs)
     - Source profiles extracted
     - Profiles diffed
     - Composition trees built
     - Conformance checks generated
  9. Results merged into report         (crates/ts/src/report.rs)
  10. JSON report written               (to --output path)

You run:  semver-analyzer konveyor typescript --repo ./my-lib --from v5 --to v6

What happens:
  Steps 1-9 same as above, then:
  10. TD rules generated                (crates/ts/src/konveyor.rs)
  11. SD rules generated                (crates/ts/src/konveyor_v2.rs)
  12. Rules consolidated & deduplicated (crates/konveyor-core/src/)
  13. Writes ruleset.yaml, rules.yaml, fix-strategies.json
```

## Crate Map

| Crate | What's in it | When you'll touch it |
|-------|-------------|---------------------|
| **`crates/core`** | Types (`Symbol`, `StructuralChange`, etc.), `Language` trait, diff engine, rename detection | Adding new change types, modifying diff logic, adding a new language |
| **`crates/ts`** | Everything TypeScript: extraction, canonicalization, source profiles, composition trees, JSX/CSS analysis, Konveyor rules | Most feature work for TypeScript/React |
| **`crates/java`** | Everything Java: extraction, annotations, module system, Maven/Gradle, Konveyor rules | Most feature work for Java |
| **`crates/llm`** | LLM integration: prompt building, CLI execution, response parsing | Modifying LLM behavioral analysis |
| **`crates/konveyor-core`** | Shared rule types, fix strategies, rule consolidation | Changing rule output format |
| **`src/`** | CLI binary, orchestrator, progress reporting, error rendering | CLI changes, pipeline coordination |

### The Golden Rule

**`crates/core/` never imports from language crates.** It defines the `Language` trait; TS and Java implement it. This separation is what makes the tool extensible to new languages.

## Key Concepts

### Composition Trees (TypeScript/React)

The most sophisticated feature. For React component libraries, the analyzer builds a tree of parent-child relationships:

```
Table (root)
  ├── Thead (Required)
  │   └── Tr (Required)
  │       └── Th (Structural)
  ├── Tbody (Required)
  │   └── Tr (Required)
  │       └── Td (Structural)
  └── Tfoot (Allowed)
```

Each edge has a strength (`Required`, `Structural`, `Wrapper`, `Allowed`) encoding two dimensions:
- **CHP** (child-must-have-parent): Does the child break without the parent?
- **PMC** (parent-must-have-child): Does the parent break without the child?

These trees drive **conformance checks** -- rules that tell consumers "if you use `<Table>`, you must wrap rows in `<Tbody>`" or "you cannot put `<Td>` directly inside `<Table>`."

The tree is built from 10 evidence signals (internal rendering, CSS selectors, grid/flex layout, React context, DOM semantics, cloneElement, BEM structure).

### Rename Detection

A 4-pass fingerprint algorithm that detects when a symbol has been renamed rather than removed-and-added. This is critical because "ButtonProps was renamed to ButtonNewProps" is much more actionable than "ButtonProps was removed" + "ButtonNewProps was added."

The algorithm uses structural fingerprints (kind + return type + parameter count) to group candidates, then picks the best match by name similarity (LCS ratio). Cross-family renames require a sibling rename between the same directories to prevent false positives.

### Migration Detection

When an interface is removed and a similar interface appears in the same package, the analyzer detects it as a migration. It computes member overlap to determine which props were preserved, which were renamed, and which were truly removed.

### Type Canonicalization (TypeScript)

TypeScript types can be written many ways (`Array<string>` vs `string[]`, `React.ReactNode` vs `ReactNode`). The canonicalizer normalizes 7 categories of variation so that type comparisons don't produce false positives.

## Development Workflow

```bash
# Build
cargo build --workspace

# Test (run before every commit)
cargo test --workspace

# Lint (CI enforces this)
cargo clippy --workspace -- -D warnings

# Format
cargo fmt --all

# Build with Java support
cargo build --workspace --features java

# Run against PatternFly (real-world validation)
./hack/run-patternfly.sh
```

## Where to Start Reading Code

If you're trying to understand a specific area:

| Area | Start Here | Then Read |
|------|-----------|-----------|
| How types are compared | `crates/core/src/diff/compare.rs` | `rename.rs`, `migration.rs` |
| How TS APIs are extracted | `crates/ts/src/extract/mod.rs` | `canon/mod.rs` |
| How React components are profiled | `crates/ts/src/source_profile/mod.rs` | `diff.rs`, `bem.rs` |
| How composition trees are built | `crates/ts/src/composition/mod.rs` | `sd_pipeline.rs` Phase B |
| How Konveyor rules are generated | `crates/ts/src/konveyor_v2.rs` | `konveyor.rs`, `konveyor-core/src/lib.rs` |
| How the orchestrator coordinates | `src/orchestrator.rs` | `src/main.rs` |
| How Java analysis works | `crates/java/src/language.rs` | `sd_pipeline.rs`, `extract/mod.rs` |

## Known Limitations

1. **ESM/CJS duplication**: When a package ships both ESM and CJS declarations, symbols appear twice. The dedup logic handles most cases but isn't perfect.
2. **Static analysis limits**: Dynamic dispatch, framework magic (dependency injection, metaprogramming), and runtime-only behavior cannot be detected.
3. **BEM cross-block**: Some CSS blocks are shared by multiple components (e.g., `.pf-v6-c-menu` used by Menu, Select, Dropdown). The analyzer has partial handling but can produce false edges.
4. **MCP server**: The `serve` subcommand is defined but not implemented.
