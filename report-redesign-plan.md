# Analysis Report Redesign Plan

## Problem

The analysis report was designed as a **breaking changes diff** (flat, per-file), but the rule generator needs a **migration plan** (hierarchical, per-component, per-package). Six compensations exist in the rule generator because the report forces downstream code to reconstruct structure that was available during analysis but discarded.

## Current Report Structure

```
AnalysisReport
├── changes: Vec<FileChanges>              ← flat, per-file
│   ├── file: PathBuf
│   ├── breaking_api_changes: Vec<ApiChange>        ← flat list, dotted symbols
│   └── breaking_behavioral_changes: Vec<BehavioralChange>  ← flat list, free-text
├── manifest_changes: Vec<ManifestChange>
├── added_files: Vec<PathBuf>              ← just paths, no metadata
└── metadata / comparison / summary
```

## Six Compensations in the Rule Generator

| # | Compensation | Lines | What's Missing |
|---|-------------|-------|----------------|
| 1 | P0-C component aggregation | 1115-1273 | No component identity — generator must parse dotted symbols, aggregate per-interface, count removals/total, infer "mostly removed" |
| 2 | `build_migration_message` full-report scans | 732-1003 | Migration target, type changes, behavioral changes for a component are scattered across files — generator scans entire report 3x for each component |
| 3 | `detect_collapsible_constant_groups` | 555-608 | No package identity on changes — generator resolves npm package from file path, groups by `(package, change_type, strategy)`, filters by threshold |
| 4 | New sibling component detection | 1622-1785 | `added_files` is just paths — generator parses stems, cross-references behavioral descriptions with regex to find `<ComponentName>` mentions |
| 5 | CSS logical property suffix extraction | 1356-1440 | Member renames not in report — generator re-scans `type_changed` entries, extracts PascalCase suffixes from snake_case names |
| 6 | `suppress_redundant_prop_rules` | 2107-2169 | No hierarchy — flat change list means property-level rules don't know they're covered by component-level rules |

## Data Computed During Analysis but Discarded

### Orchestrator
- `old_surface` / `new_surface` (full `ApiSurface`) — available in `SharedFindings`, not in report
- `BehavioralBreak.call_path` — dropped during conversion to `BehavioralChange`
- `BehavioralBreak.confidence` — dropped during merge
- `BehavioralBreak.evidence` (TestDelta, JsxDiff, LlmOnly) — dropped entirely
- Package info (name, version) — computed from `package.json` at rule-gen time, not in report

### Diff Engine
- `Symbol.members` (full tree) — diff produces flat `StructuralChange`, hierarchical member tree lost
- `SymbolAdded` changes — computed but filtered out (report only tracks breaking)
- Full `StructuralChange` detail — 35 `change_type` variants collapsed to 5 `ApiChangeType` values
- `RenameMatch.similarity` — similarity score discarded

## Proposed Report v2 Structure

```
AnalysisReport
├── comparison: Comparison
├── summary: Summary
├── packages: Vec<PackageChanges>                    ← NEW: group by package
│   ├── name: "@patternfly/react-core"
│   ├── old_version: "5.4.0"
│   ├── new_version: "6.1.0"
│   ├── components: Vec<ComponentSummary>             ← NEW: per-component view
│   │   ├── name: "Modal"
│   │   ├── interface_name: "ModalProps"
│   │   ├── status: Modified | Removed | Added
│   │   ├── property_summary: PropertySummary
│   │   │   ├── total: 14
│   │   │   ├── removed: 12
│   │   │   ├── renamed: 1
│   │   │   ├── type_changed: 1
│   │   │   └── removal_ratio: 0.86
│   │   ├── removed_properties: Vec<RemovedProperty>
│   │   │   └── { name, old_type, description }
│   │   ├── type_changes: Vec<TypeChange>
│   │   │   └── { property, before, after }
│   │   ├── migration_target: Option<MigrationTarget>
│   │   ├── behavioral_changes: Vec<BehavioralChange>  ← pre-grouped per-component
│   │   ├── child_components: Vec<ChildComponent>      ← NEW: discovered siblings
│   │   │   └── { name, status: Added|Modified, known_props }
│   │   └── source_files: Vec<PathBuf>
│   ├── constants: Vec<ConstantGroup>                 ← NEW: pre-grouped constants
│   │   ├── change_type: TypeChanged | Removed
│   │   ├── count: 2020
│   │   ├── common_prefix_pattern: "^(c_|global_|chart_)\\w+$"
│   │   ├── strategy_hint: "CssVariablePrefix"
│   │   └── suffix_renames: Vec<{from, to}>           ← NEW: pre-extracted
│   └── added_components: Vec<AddedComponent>          ← NEW: structured additions
│       └── { name, path, package, directory }
├── manifest_changes: Vec<ManifestChange>
├── member_renames: HashMap<String, String>            ← NEW: surfaced from analysis
└── metadata: AnalysisMetadata
```

## Three Structural Changes

### 1. Package-level grouping

**Add `package: Option<String>` to `ApiChange` and group `FileChanges` by package.**

During analysis, `resolve_npm_package()` is already called. Instead of discarding the result, store it on each `ApiChange`. In the report, add a `packages: Vec<PackageChanges>` section that groups all changes by resolved package.

**Eliminates:**
- `resolve_npm_package()` at rule-gen time
- `build_package_name_cache()` / `build_package_info_cache()` duplication
- Package inference in `detect_collapsible_constant_groups`

### 2. Component-level summaries

**Add a `components: Vec<ComponentSummary>` section with pre-aggregated data.**

During analysis (after the diff but before writing the report), aggregate:
- Property-level changes by parent interface (what P0-C does now)
- Migration target data (what `build_migration_message` searches for)
- Related behavioral changes (what the message builder cross-references)
- Child/sibling components (what new sibling detection discovers)
- Removal ratio and severity (what the `mostly_removed` check computes)

**Eliminates:**
- P0-C aggregation loop
- `build_migration_message` full-report scans (3x per component)
- New sibling detection with directory parsing and behavioral regex
- `suppress_redundant_prop_rules` (hierarchy makes redundancy obvious)

### 3. Preserve analysis artifacts

**Include `member_renames`, `behavioral_evidence`, and `confidence` in the report.**

- `member_renames: HashMap<String, String>` — computed by `analyze_token_members()`, currently a side-channel. Surface as a top-level report field.
- `BehavioralChange` — add `confidence: f64`, `evidence_type: String`, and `referenced_components: Vec<String>` (structured, not free-text regex).
- `ConstantGroup` — pre-group bulk constant changes by package + change type + strategy. Include `suffix_renames` extracted during analysis.

**Eliminates:**
- `analyze_token_members` re-scanning at rule-gen time
- CSS suffix extraction from member_renames
- Free-text parsing of behavioral descriptions for component names
- `detect_collapsible_constant_groups` threshold scanning

## Impact on Rule Generator

| Current Code | With Report v2 |
|-------------|----------------|
| `generate_rules()` — 900 lines with 6 compensations | ~300 lines reading pre-structured data |
| P0-C extension (50 lines) | Read `component.removal_ratio > 0.5` from report |
| `build_migration_message()` (130 lines, 3 full-report scans) | Read `component.migration_target`, `.behavioral_changes`, `.type_changes`, `.child_components` directly |
| `detect_collapsible_constant_groups()` (60 lines) | Read `package.constants` pre-grouped section |
| `suppress_redundant_prop_rules()` (65 lines) | Unnecessary — hierarchy means property rules know their parent |
| New sibling detection (160 lines) | Read `component.child_components` |
| CSS suffix extraction (80 lines) | Read `package.constants[].suffix_renames` |

## Migration Path

### Phase 1: Add package and component fields (non-breaking)
- Add `package: Option<String>` to `ApiChange`
- Add `components: Vec<ComponentSummary>` to `AnalysisReport` with `#[serde(default)]`
- Populate during `build_report()` in the orchestrator
- Rule generator reads new fields when present, falls back to old logic when absent
- Old reports without these fields still work (serde default)

### Phase 2: Populate component summaries
- After diff, aggregate property changes by parent interface
- Cross-reference behavioral changes by component name
- Discover child components from `added_files` + directory analysis
- Compute removal ratios and severity
- Attach migration targets inline

### Phase 3: Simplify rule generator
- Replace P0-C aggregation with `for component in report.components`
- Replace `build_migration_message` scans with direct field reads
- Replace constant grouping with `for group in package.constants`
- Remove suppression functions (hierarchy handles it)
- Remove sibling detection (pre-computed)

### Phase 4: Enrich with preserved artifacts
- Surface `member_renames` in report
- Add structured `referenced_components` to behavioral changes
- Add confidence scores to behavioral changes
- Pre-compute constant groups with suffix renames

## Priority
High — this is the foundational fix that would make all future rule generation improvements significantly easier. Every new feature currently requires adding another "compensation scan" to the rule generator. With a proper report structure, new features read pre-computed data.

## Files Affected
- `crates/core/src/types/report.rs` — new types (ComponentSummary, PackageChanges, etc.)
- `src/orchestrator.rs` — aggregate data after diff, before writing report
- `src/main.rs` — `build_report()` populates new fields
- `src/konveyor/mod.rs` — simplify `generate_rules()` to read new fields
