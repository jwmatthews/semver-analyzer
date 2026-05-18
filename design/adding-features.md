# Adding Features & Fixing Bugs

Step-by-step guides for common modifications.

## Decision: Where Does My Change Go?

| If you're changing... | It belongs in... | Why |
|----------------------|------------------|-----|
| A type used by multiple languages | `crates/core/src/types/` | Language-agnostic types |
| The structural diff algorithm | `crates/core/src/diff/` | Language-agnostic diff engine |
| How TypeScript APIs are extracted | `crates/ts/src/extract/` | TS-specific extraction |
| How React components are analyzed | `crates/ts/src/source_profile/` or `sd_pipeline.rs` | TS source-level analysis |
| Component hierarchy detection | `crates/ts/src/composition/` | TS composition trees |
| How Java APIs are extracted | `crates/java/src/extract/` | Java-specific extraction |
| Java source-level analysis | `crates/java/src/sd_pipeline.rs` | Java SD pipeline |
| LLM prompts or response parsing | `crates/llm/src/` | Language-agnostic LLM |
| Konveyor rule format/conditions | `crates/konveyor-core/src/` | Shared rule types |
| TS-specific Konveyor rules | `crates/ts/src/konveyor.rs` or `konveyor_v2.rs` | TS rule generation |
| Java-specific Konveyor rules | `crates/java/src/konveyor.rs` | Java rule generation |
| CLI arguments or output | `src/cli/mod.rs` or `src/main.rs` | Binary entry point |
| Pipeline orchestration | `src/orchestrator.rs` | Analysis coordinator |

**Critical invariant**: `crates/core/` must NEVER import from `crates/ts/`, `crates/java/`, or `crates/llm/`.

## Adding a New Structural Change Type

The diff engine uses `StructuralChangeType` with 5 lifecycle variants and `ChangeSubject` to describe what changed. To detect a new kind of change:

1. **Check if existing types cover it**. Most changes can be expressed as `Changed(ChangeSubject::X)` with appropriate `before`/`after` strings.

2. **If you need a new `ChangeSubject` variant**:
   - Add variant to `ChangeSubject` in `crates/core/src/types/change_subject.rs`
   - Update `to_api_change_type()` in `report.rs` to map it
   - Add detection logic in the appropriate `diff/compare.rs` function
   - Add test in `diff/tests.rs`

3. **If you need language-specific comparison logic**:
   - Add a method to `LanguageSemantics` trait (with a default implementation)
   - Override in the language crate's `Language` impl
   - The diff engine calls language semantics methods at specific points

## Adding a New Source-Level Category (TypeScript)

1. Add variant to `SourceLevelCategory` in `crates/ts/src/sd_types.rs`
2. Add a corresponding handler in `source_profile/diff.rs` (called from `diff_profiles()`)
3. Add detection/extraction logic in the appropriate `source_profile/` module if the profile needs new fields
4. Add a Konveyor v2 rule generator in `konveyor_v2.rs` (called from `generate_sd_rules()`) if the change should produce migration rules
5. Wire the new detection into `sd_pipeline.rs` if it requires pipeline-level orchestration beyond profile diffing
6. Add test assertions in `crates/ts/tests/baseline_behavioral.rs`

## Adding a New Source-Level Category (Java)

1. Add variant to `JavaSourceCategory` in `crates/java/src/sd_types.rs`
2. Add profile fields to `JavaClassProfile` or `MethodProfile` in `sd_types.rs` if the detection requires new extracted data
3. Add extraction logic in `sd_pipeline.rs` profile extraction (the `extract_class_profile()` / `extract_method_profile()` functions)
4. Add diff logic in `sd_pipeline.rs` `diff_class_profiles()` or `diff_method_profiles()`
5. Add Konveyor rule generation in `konveyor.rs` `generate_sd_rules()` to produce migration rules from the new category
6. Add test in `crates/java/tests/baseline_sd.rs` with snapshot assertions

## Adding a New Composition Tree Signal (TypeScript)

The composition tree builder (`crates/ts/src/composition/mod.rs`) has ~20 distinct signal steps (1, 1.5, 2, 3, 3b, 3c, 4, 5, 5.5, 6, 7, 8, 8.5, 8.6, 8.7, 8.8, 9, 9.5, 9.6, 10). To add a new signal:

1. Identify where in the pipeline your signal should run (ordering matters -- earlier signals can be refined by later ones)
2. Add your signal step after the appropriate existing step
3. Your step should add or strengthen `CompositionEdge` entries in the edges collection
4. Use `EdgeStrength::combine()` when strengthening an existing edge
5. Add unit tests
6. Validate against the PatternFly ground truth in `design/composition-ground-truth.md` (78 known-correct edges)

### EdgeStrength Selection Guide

| Choose... | When... |
|-----------|---------|
| `Required` | Both parent and child break without each other (e.g., `<Table>` and `<Tr>`) |
| `Structural` | Child breaks without parent, but parent works without child (e.g., `<Td>` needs `<Tr>`) |
| `Wrapper` | Parent expects child, but child can exist alone (e.g., `<Card>` expects `<CardBody>`) |
| `Allowed` | Valid nesting but neither side strictly requires the other |

## Adding a New Konveyor Rule Type

**v1 vs v2 split:** TypeScript Konveyor rule generation is split across two files:
- `crates/ts/src/konveyor.rs` (v1) -- generates rules from **TD pipeline** results (structural API diff): removed/renamed symbols, type changes, removed union values, new-sibling rules, import-deprecated rules. Operates on `AnalysisReport<TypeScript>`.
- `crates/ts/src/konveyor_v2.rs` (v2) -- generates rules from **SD pipeline** results: composition changes, conformance checks, context dependencies, prop-to-child migration, test impact, CSS removal, prop-attribute-override. Operates on `SdPipelineResult`.

Some rules also require orchestration in `src/main.rs` where both TD and SD results are available (e.g., family strategy rules that combine structural changes with composition trees).

### For TypeScript SD rules (`crates/ts/src/konveyor_v2.rs`)

1. Create a function `generate_my_rules(report, sd, pkg_cache) -> Vec<KonveyorRule>`
2. Build `KonveyorCondition` (usually `FrontendReferenced` with pattern + importPath)
3. Build `FixStrategyEntry` with appropriate category, summary, and guidance
4. Call your function from `generate_sd_rules()`
5. Add snapshot test

### For Java rules (`crates/java/src/konveyor.rs`)

1. Similar pattern but use `KonveyorCondition::JavaReferenced` or `JavaDependency`
2. Call from `generate_rules()` or `generate_sd_rules()`
3. Add test in `crates/java/tests/baseline_konveyor.rs`

## Adding a New Language

This is the largest possible change. Follow the `Language` trait contract:

1. **Create new crate**: `crates/your_lang/` with `Cargo.toml`, `src/lib.rs`
2. **Define the 6 associated types** on the `Language` trait:
   - `SymbolData` — per-symbol metadata (e.g., `YourLangSymbolData` struct)
   - `Category` — behavioral change categories
   - `ManifestChangeType` — package manifest change types
   - `Evidence` — evidence data for behavioral changes
   - `ReportData` — language-specific report data
   - `AnalysisExtensions` — pipeline extensions (e.g., SD results)
3. **Implement extraction**: `extract()` method that produces `ApiSurface<YourLangSymbolData>`
4. **Implement `LanguageSemantics`**: At minimum, `is_member_addition_breaking()`, `same_family()`, `visibility_rank()`
5. **Implement `MessageFormatter`**: `describe()` for human-readable change descriptions
6. **Implement remaining `Language` methods**: diff parsing, test discovery, manifest diffing, report building
7. **Add CLI subcommand**: In `src/cli/mod.rs` and `src/main.rs`
8. **Add Konveyor rule generation**: language-specific rule builder
9. **Add to workspace**: In root `Cargo.toml`, behind a feature flag

Reference: `crates/java/` is a good model (simpler than TypeScript, covers all the required trait methods).

## Fixing a False Positive in Rename Detection

The rename detector lives in `crates/core/src/diff/rename.rs`.

1. **Identify the fingerprint pass** that produces the false positive (1-4)
2. **Check thresholds**: same-family 0.15, cross-family 0.50, ambiguous 0.45
3. **Check the cross-family sibling guard**: requires a confirmed sibling rename between same directories
4. **Add a test case** in `diff/tests.rs` that reproduces the false positive
5. **Adjust thresholds or add filtering logic**
6. **Validate against ground truth** in `design/rename-detector-verification.md` (15 true renames, 28 false renames)

## Fixing a False Positive in Migration Detection

`crates/core/src/diff/migration.rs`:

1. **Check adaptive thresholds**: member count -> required overlap (1-3:1, 4-6:2, 7+:3)
2. **Check MIN_OVERLAP_RATIO**: 0.25
3. **Check small-set fuzzy matching**: similarity 0.60, max 8 per side
4. **Add test case** in `crates/ts/tests/baseline_migration.rs`

## Common Gotchas

### Type Canonicalization
If you see type comparison failures, check `crates/ts/src/canon/mod.rs`. The canonicalizer handles edge cases like:
- `Array<T>` vs `T[]`
- `React.ReactNode` vs `import("react").ReactNode` vs `ReactNode`
- `ReactElement<any>` vs `ReactElement` (default generic stripped)
- `T | never` -> `T`, `T & unknown` -> `T`

### Dist Deduplication
If you see duplicate symbols, check the dist dedup logic in `crates/ts/src/extract/mod.rs`. Priority: `esm > mjs > es > js > cjs > commonjs > lib`.

### BEM Independence
Some components share a CSS class prefix but are independent blocks (e.g., `pf-v6-c-menu` used by both Menu and Select). The composition tree builder handles this via BEM block independence checks.

### Java Overload Disambiguation
The Java diff parser uses `overload_key` (name + parameter types) to match overloaded methods. If you see incorrect method matching, check `crates/java/src/diff_parser/mod.rs`.
