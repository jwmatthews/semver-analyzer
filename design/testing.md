# Testing Guide

## Test Organization

| Location | Type | Count | What It Tests |
|----------|------|-------|---------------|
| `crates/core/src/diff/tests.rs` | Unit | ~65 | Diff engine: all change types, renames, relocations, migrations |
| `crates/ts/tests/baseline_diff.rs` | Integration | 58 | TypeScript structural diff with TS semantics |
| `crates/ts/tests/baseline_manifest.rs` | Integration | 20 | package.json diffing |
| `crates/ts/tests/baseline_behavioral.rs` | Integration | 19 | JSX diff and CSS scan |
| `crates/ts/tests/baseline_migration.rs` | Integration | 7 | Migration detection |
| `crates/java/tests/baseline_diff.rs` | Integration | 15 | Java structural diff |
| `crates/java/tests/baseline_konveyor.rs` | Integration | 9 | Java rule generation |
| `crates/java/tests/baseline_manifest.rs` | Integration | 7 | pom.xml / build.gradle diffing |
| `crates/java/tests/baseline_sd.rs` | Integration | 11 | Java SD pipeline profile diffing |
| `crates/konveyor-core/tests/token_rename_pipeline.rs` | Integration | 3 | Full rename pipeline with 4028 real entries |
| `crates/ts/src/snapshots/` | Snapshot | 12 | Konveyor rule YAML output |
| `crates/java/tests/snapshots/` | Snapshot | ~42 | Java structural diff, konveyor, manifest, and SD YAML output |
| Inline `#[test]` across all crates | Unit | ~900+ | Individual function tests |

## Running Tests

```bash
# All tests
cargo test --workspace

# Specific crate
cargo test -p semver-analyzer-core
cargo test -p semver-analyzer-ts
cargo test -p semver-analyzer-java
cargo test -p semver-analyzer-konveyor-core

# With Java feature
cargo test --workspace --features java

# Single test
cargo test -p semver-analyzer-ts test_name

# Update snapshots (when intentionally changing output)
cargo insta review
```

## Test Helpers

### TypeScript (`crates/ts/tests/helpers.rs`)

Factory functions for building test fixtures:

```rust
// Build a symbol with TsSymbolData
sym("name", SymbolKind::Interface, members)

// Build function symbol with signature
func("name", params, "return_type")

// Build parameters
param("name", "string")       // required
opt_param("name", "string")   // optional
rest_param("name", "string")  // variadic

// Build API surface
surface(vec![sym1, sym2])

// Build interface member (property)
mk_prop("name", "string")

// Build enum member
enum_member("name", "value")

// Build interface with members
make_interface("InterfaceName", "file.ts", &["x"])
```

### Java (`crates/java/tests/helpers.rs`)

```rust
// Build Java symbols
java_class("ClassName", members)
java_interface("InterfaceName", members)
java_enum("EnumName", members)
java_method("methodName", params, "returnType")
java_field("fieldName", "fieldType")
java_constant("CONSTANT_NAME", "value")

// Add Java-specific metadata
with_annotation(symbol, "Override")
with_final(symbol)
with_sealed(symbol)
with_throws(symbol, vec!["IOException"])
with_extends(symbol, "BaseClass")
with_implements(symbol, vec!["Serializable"])
with_abstract(symbol)
```

### Java Test Infrastructure (`crates/java/tests/`)

The Java crate has four integration test files covering all analysis pipelines:

- **`baseline_diff.rs`** (15 scenarios) — Structural diff: class/interface/enum changes, method signature changes, sealed permits, annotation diffs, overload disambiguation, generic type parameter changes.
- **`baseline_konveyor.rs`** (9 scenarios) — Konveyor rule generation from both TD structural changes and SD pipeline results. Tests rule conditions, fix strategies, and effort classification.
- **`baseline_manifest.rs`** (7 scenarios) — Maven `pom.xml` and Gradle `build.gradle` manifest diffing: dependency version changes, dependency additions/removals, plugin changes.
- **`baseline_sd.rs`** (11 scenarios) — Java SD pipeline: annotation changes, serialization field changes, exception clause changes, synchronization changes, override detection, constructor dependency changes.

**`helpers.rs`** provides `NormalizedChange` and `NormalizedManifestChange` structs for snapshot-friendly comparison. These strip non-deterministic fields (like internal IDs) and normalize enum variants to strings for readable YAML snapshots. Also provides construction helpers: `java_class`, `java_interface`, `java_enum`, `java_method`, `java_field`, `java_constant`, `with_annotation`, `with_sealed`, `with_throws`, etc.

Snapshot files (~42) live in `crates/java/tests/snapshots/` and cover all four test suites.

### Core (`crates/core/src/lib.rs`)

`TestLang` -- a minimal `Language` implementation with `()` for all associated types. Used for core diff engine unit tests.

`TsLikeTestSemantics` (in `diff/tests.rs`) -- implements TypeScript-like semantic rules for diff tests: star re-export filtering, deprecated path canonicalization, relocation classification.

## Snapshot Testing

Uses the `insta` crate for YAML snapshot comparisons. Snapshot files live in `src/snapshots/` directories.

### How Snapshots Work

1. Test calls `insta::assert_yaml_snapshot!(value)` or `insta::assert_snapshot!(string)`
2. First run: creates `.snap.new` file with actual output
3. Run `cargo insta review` to accept/reject
4. Subsequent runs: compares against stored `.snap` file

### When to Update Snapshots

Update snapshots when you **intentionally** change output format. If a snapshot test fails unexpectedly, investigate the root cause before updating.

```bash
# Review pending snapshot changes interactively
cargo insta review

# Accept all pending changes (use with caution)
cargo insta accept
```

## Writing New Tests

### For a New Structural Diff Feature

1. Add a test in `crates/core/src/diff/tests.rs` or `crates/ts/tests/baseline_diff.rs`
2. Use `helpers::surface()` to build old and new API surfaces
3. Call `diff_surfaces_with_semantics()` with appropriate semantics
4. Assert with `insta::assert_yaml_snapshot!(changes)`

```rust
#[test]
fn test_my_new_feature() {
    let old = surface(vec![
        make_interface("MyInterface", vec![mk_prop("old_prop", "string")]),
    ]);
    let new = surface(vec![
        make_interface("MyInterface", vec![mk_prop("new_prop", "string")]),
    ]);
    let semantics = TypeScript::default();
    let changes = diff_surfaces_with_semantics(&old, &new, &semantics);
    insta::assert_yaml_snapshot!(changes);
}
```

### For a New SD Pipeline Feature

1. Add test in the relevant module's test section
2. Build `ComponentSourceProfile` instances with the relevant fields set
3. Call `diff_profiles()` and check the output
4. For composition tree changes, build profiles with `rendered_components` and CSS profiles

### For a New Konveyor Rule

1. Add test in `crates/ts/src/konveyor_v2.rs` (inline tests) or create fixture in `tests/`
2. Build an `AnalysisReport` or `SdPipelineResult` with the triggering condition
3. Call the rule generator and snapshot the output

### For Java Features

Follow the same pattern using Java helpers. Key difference: Java tests use `tree-sitter` directly rather than needing `.d.ts` generation.

## CI Integration

Tests run in `.forgejo/workflows/ci.yml`:
```yaml
- cargo fmt --all -- --check
- cargo clippy --workspace -- -D warnings
- cargo test --workspace
- cargo build --workspace
```

All tests must pass before merge. Clippy warnings are treated as errors.

## Real-World Validation

The project validates against real-world repositories:
- **PatternFly React** v5.4.0 -> v6.4.1 (56k+ symbols, 17k+ breaking changes)
- Test fixtures in `crates/core/tests/fixtures/`

The `hack/run-patternfly.sh` script automates the PatternFly analysis for regression testing.
