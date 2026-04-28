# semver-analyzer-java

Java language plugin for the semver-analyzer. Implements the `Language` trait from `semver-analyzer-core` and provides complete Java-specific analysis: API surface extraction, source-level behavioral diff (SD pipeline), manifest diffing, call graph walking, test discovery, Konveyor rule generation, and report building.

Uses [tree-sitter-java](https://github.com/nickel-org/tree-sitter-java) for all AST operations. No build step required for basic analysis; optional Maven/Gradle build support for generated source detection.

## Modules

### `extract` -- API Surface Extraction

`JavaExtractor` parses `.java` source files directly with tree-sitter and extracts the public API surface:

- **Type declarations** -- classes, interfaces, enums, records, annotation types
- **Members** -- methods, constructors, fields, constants, enum constants, annotation elements
- **Modifiers** -- public/protected/package-private visibility, final, sealed, non-sealed, abstract, synchronized, transient, volatile, native
- **Annotations** -- name, qualified name (via import resolution including wildcard imports), key-value attributes
- **Type parameters** -- with bounds (`<T extends A & B>`)
- **Records** -- canonical constructors (including compact constructors), component accessors
- **Enums** -- constants with constructor arguments
- **Nested types** -- inner classes, static nested classes, with correct qualified names

### `extract/module_info` -- Module System Parsing

Parses `module-info.java` files and extracts all 7 directive types: `requires`, `requires transitive`, `exports`, `exports ... to`, `opens`, `opens ... to`, `provides ... with`, and `uses`.

### `sd_pipeline` -- Source-Level Diff (SD Pipeline)

The default analysis pipeline. Deterministic AST-based behavioral change detection:

1. **Phase A** -- Find changed `.java` files via `git diff`, extract `JavaClassProfile` at both refs, diff profiles to produce `JavaSourceChange` entries
2. **Phase B** -- Extract all profiles at the new version for complete picture
3. **Phase B.5** -- Resolve inheritance chains (transitive Serializable detection)
4. **Phase B1** -- Build inheritance trees, detect hierarchy breakages
5. **Phase B3** -- Module system diff (`module-info.java` directive changes)

Detects 20 categories of source-level changes: annotation changes, delegation changes, exception changes, synchronization changes, serialization compatibility, override removal, constructor dependency changes, final/sealed/inheritance changes, module system changes, and native modifier changes.

### `index` -- Cross-File Java Index

`JavaIndex` pre-parses all `.java` files using tree-sitter and builds lookup tables for:

- Type declarations (simple name to qualified name + file)
- Method declarations (name, enclosing type, visibility, body, signature)
- Import maps per file
- Package names per file

Enables `find_callers` (call graph walking for BU pipeline) and `find_references` (cross-file symbol usage search).

### `diff_parser` -- Changed Function Detection

`JavaDiffParser` parses `git diff` between two refs to identify functions with changed implementations. Handles method overloading via parameter type disambiguation. Extracts `static final` constant values for value-change detection. Uses string-literal-aware comment stripping for accurate body normalization.

### `test_analyzer` -- Test Discovery and Assertion Diffing

`JavaTestAnalyzer` discovers test files using 3 strategies:

1. Maven/Gradle standard layout (`src/main/java/` to `src/test/java/`)
2. Sibling test files (same directory)
3. Recursive search in `test/` directories

Supports 7 naming patterns: `FooTest`, `FooTests`, `FooIT`, `FooITCase`, `FooSpec`, `TestFoo` (prefix), and exact name match.

Assertion detection covers JUnit 4/5, AssertJ (50+ methods), Hamcrest matchers, TestNG, Mockito verify, and Google Truth.

### `manifest` -- Maven/Gradle Manifest Diffing

- **pom.xml** -- event-based XML parsing with `quick-xml`. Diffs project identity, parent version, dependencies (added/removed/version changed/scope changed), and properties.
- **build.gradle** / **build.gradle.kts** -- regex-based dependency extraction for implementation, api, compileOnly, runtimeOnly, testImplementation, and annotationProcessor configurations.

### `konveyor` -- Konveyor Rule Generation

Generates [Konveyor](https://www.konveyor.io/) migration rules from analysis reports:

- **TD rules** (`generate_rules_with_config`) -- import relocation, rename, removal, type changed, signature changed, visibility changed, dependency update
- **SD rules** (`generate_sd_rules`) -- annotation removed/changed, synchronization removed, exception added, serialization break, final/sealed added, inheritance changed, native removed, delegation changed, module export removed

All rules are parameterized via `JavaKonveyorConfig` (project name, rule ID prefix, migration guide URL) -- not hardcoded to any specific framework.

### `report` -- Report Building

Builds `AnalysisReport<Java>` from raw `AnalysisResult<Java>`. Resolves git SHAs, counts commits between refs, generates timestamps, groups changes by file, and wires SD pipeline results into the report extensions.

### `worktree` -- Git Worktree Management

`JavaWorktreeGuard` provides RAII git worktree lifecycle management with optional build support:

1. Creates worktree via core's `WorktreeGuard`
2. Auto-detects build system (pom.xml for Maven, build.gradle for Gradle)
3. Optionally runs build command with configurable JAVA_HOME
4. Records build warnings as `ExtractionWarning` for degradation tracking
5. Cleans up on drop

### `language` -- Java Implementation

The `Java` struct implements three core traits:

- **`Language`** -- binds associated types (`JavaCategory`, `JavaManifestChangeType`, `JavaEvidence`, `JavaReportData`, `JavaAnalysisExtensions`), runs SD pipeline via `run_extended_analysis`, supports per-ref build configuration
- **`LanguageSemantics`** -- Java-specific breaking-change rules (abstract method additions on interfaces, default method exceptions, annotation type element defaults, package-based family grouping, visibility ranking)
- **`MessageFormatter`** -- delegates to change descriptions

## Usage

```rust
use semver_analyzer_java::{Java, JavaRefBuildConfig};
use semver_analyzer_core::Language;

let java = Java::new();

// Extract API surface at a git ref
let surface = java.extract(repo_path, "v3.2.0", None)?;

// With build support
let config = JavaRefBuildConfig {
    build_command: Some("mvn compile -DskipTests".into()),
    ..Default::default()
};
let java_with_build = Java::with_ref_config(config);
let surface = java_with_build.extract(repo_path, "v4.0.0", None)?;
```

## Dependencies

| Crate | Purpose |
|-------|---------|
| `semver-analyzer-core` | Shared traits and types |
| `semver-analyzer-konveyor-core` | Shared Konveyor rule types |
| `tree-sitter`, `tree-sitter-java` | Java source parsing |
| `quick-xml` | pom.xml parsing |
| `serde`, `serde_json` | Serialization |
| `clap` | CLI argument parsing |
| `anyhow`, `thiserror` | Error handling |
| `regex` | Gradle dependency parsing |
| `tracing` | Structured logging |

## License

Apache-2.0
