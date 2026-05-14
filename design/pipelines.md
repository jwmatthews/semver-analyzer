# Pipeline Deep Dive

semver-analyzer has three analysis pipelines. The TD pipeline always runs. Either SD (default) or BU runs alongside it.

## TD Pipeline (Top-Down, Structural)

**Always runs. Source: `crates/core/src/diff/`**

Compares `.d.ts` (TypeScript) or `.java` (Java) API surfaces structurally. This is the backbone of the analysis.

### Extraction Phase

1. Create git worktree for each ref
2. Install dependencies + build (TypeScript: `npm install` + `tsc --declaration`; Java: `mvn compile`)
3. Parse output files into `ApiSurface<M>` (vector of `Symbol<M>` with members, signatures, types)
4. TypeScript uses OXC parser (`crates/ts/src/extract/`); Java uses tree-sitter (`crates/java/src/extract/`)

### 6-Phase Diff Engine (`crates/core/src/diff/mod.rs`)

Given `old_surface` and `new_surface`, produces `Vec<StructuralChange>`:

**Phase 1 -- Relocation Detection** (`relocate.rs`)
- Canonical path matching strips `/deprecated/` and `/next/` prefixes
- Matches removed symbols to added symbols at equivalent canonical paths
- 5 relocation types: `MovedToDeprecated`, `PromotedFromDeprecated`, `PromotedFromNext`, `MovedToNext`, `Relocated`

**Phase 2 -- Rename Detection** (`rename.rs`)
- 4-pass fingerprint matching algorithm:
  1. **Exact fingerprint**: Match by `MemberFingerprint` (kind + return_type + is_optional + param_count), then rank by name similarity
  2. **Structural fingerprint**: Normalized types (PascalCase -> `_T_`, param names -> `_p_`)
  3. **Deep structural**: Also normalizes string literal values -> `_V_`
  4. **Name-only fallback**: For same-interface properties with similarity >= 0.6
- Similarity thresholds: same-family 0.15, cross-family 0.50, ambiguous groups 0.45
- MAX_GROUP_SIZE cap of 50 per fingerprint bucket
- Cross-family sibling validation guard (requires a sibling rename between same directories)

**Phase 2b -- Token Rename Detection**
- For constants/variables: Jaccard similarity on `_`-split lowercased name segments (min 0.6)
- Uses inverted index for efficiency
- Value-based fallback when names diverge completely

**Phase 3 -- Unmatched Symbols**
- Remaining unmatched removed symbols -> `Removed` changes
- Remaining unmatched added symbols -> `Added` changes

**Phase 4 -- Compare Matched Symbols** (`compare.rs`)
- Exact qualified_name matches diffed member-by-member
- Recursive: container members are compared via the same rename + diff pipeline
- Checks: visibility, modifiers (readonly/abstract/static/accessor), hierarchy (extends/implements), signatures (parameters, return type, type parameters), union literals, language-specific data

**Phase 5 -- Migration Detection** (`migration.rs`)
- For removed interfaces/classes: find potential replacement in same family
- Member overlap analysis with adaptive thresholds:
  - 1-3 members: need 1 overlap; 4-6: need 2; 7+: need 3
  - MIN_OVERLAP_RATIO: 0.25
- Small-set fuzzy prop matching (up to 8 per side, 0.60 similarity + type compatibility)
- Produces `MigrationTarget` with `matching_members` and `overlap_ratio`

**Phase 6 -- Post-processing**
- Language-specific cleanup via `LanguageSemantics::post_process()`
- TypeScript: deduplicates `default` export changes when named sibling exists

### Output

`Vec<StructuralChange>` where each change has:
- `change_type: StructuralChangeType` (5 variants: `Added(ChangeSubject)`, `Removed(ChangeSubject)`, `Changed(ChangeSubject)`, `Renamed { from, to }`, `Relocated { from, to }`)
- `symbol`, `qualified_name`, `kind`, `package`, `before`, `after`, `description`, `is_breaking`, `migration_target`

---

## SD Pipeline (Source-Level Diff, Deterministic)

**Default pipeline. Source: `crates/ts/src/sd_pipeline.rs` (TS), `crates/java/src/sd_pipeline.rs` (Java)**

Analyzes source code changes at the AST level without LLM. Produces deterministic, reproducible results.

### TypeScript SD Pipeline

**Phase A -- Changed File Analysis**
1. Find changed `.tsx` files via `git diff --name-only`
2. For each changed file, extract `ComponentSourceProfile` at both refs (25+ fields: rendered elements, ARIA attributes, prop defaults, CSS tokens, BEM structure, portal usage, contexts, etc.)
3. Diff profiles to produce `SourceLevelChange` entries across 15 categories

**Phase A.5 -- Deprecated Migration Diffing**
- Pairs deprecated components with their replacements
- Diffs the replacement's old vs new profile against the deprecated component

**Phase A.7 -- Transitive Behavioral Changes**
- Traces changed helper functions through import chains
- If a helper changed and is imported by a component, generates transitive change entries

**Phase B -- Full Composition Analysis**
1. Enumerate ALL component files at the to-ref
2. Extract source profiles for every component
3. Build composition trees via `build_composition_tree_v2()` (10-step signal algorithm)
4. Diff old vs new composition trees
5. Generate conformance checks

**Composition Tree Builder** (`crates/ts/src/composition/mod.rs`):
10 signal steps combine evidence to build parent-child edges:
1. Internal rendering (from `rendered_components` in JSX)
1.5. Delegate tree projection
2. CSS direct-child selectors (`.parent > .child`)
3. CSS grid parent-child (grid-template vs grid-column)
4. CSS flex context (flex container -> flex items)
5. CSS descendant selectors (`.parent .child`)
5.5. CSS layout children
6. React context (provider/consumer)
7. DOM nesting (`<ul>` -> `<li>`)
8. cloneElement threading
8.5-8.7. BEM orphan fallback, secondary block, prop-passed detection
9. Suppress root edges when intermediate exists
10. Drop unconnected members

**EdgeStrength** (4-valued enum modeling two dimensions):
- CHP (child-must-have-parent): does the child break without the parent?
- PMC (parent-must-have-child): does the parent break without the child?

| Variant | CHP | PMC | Example |
|---------|-----|-----|---------|
| `Required` | true | true | `<Table>` requires `<Tr>`, `<Tr>` requires `<Table>` |
| `Structural` | true | false | `<Td>` must be in `<Tr>`, but `<Tr>` can exist without `<Td>` |
| `Wrapper` | false | true | `<CardBody>` can exist alone, but `<Card>` expects it |
| `Allowed` | false | false | Valid nesting but neither side requires the other |

**Conformance Checks** (4 types generated from edge strength):
- `MissingIntermediate` / `notParent` -- CHP=true edges
- `InvalidDirectChild` -- parent has `Wrapper`/`Required` children only
- `RequiresChild` -- PMC=true edges
- `ExclusiveWrapper` -- single-child exclusive containers

### Java SD Pipeline

**Phase A**: Find changed `.java` files, extract `JavaClassProfile` at both refs, diff them
**Phase B**: Extract all profiles, resolve inheritance chains (transitive `Serializable` detection)
**Phase B.5**: Build inheritance summary
**Phase B3**: Module system diff (`module-info.java` directives)

22 source-level change categories including: annotation changes, synchronization, exceptions, serialization, override, constructor dependencies, module exports, final/sealed, inheritance, native.

### Output

TypeScript: `SdPipelineResult` with ~20 fields including `source_level_changes`, `composition_trees`, `composition_changes`, `conformance_checks`, component props/types inventories, CSS inventories, deprecated replacements.

Java: `JavaSdPipelineResult` with `source_level_changes`, profiles, module changes, inheritance summary.

---

## BU Pipeline (Bottom-Up, Behavioral)

**Opt-in via `--behavioral`. Source: `src/orchestrator.rs` + `crates/llm/`**

Uses LLM to detect behavioral changes not visible from type signatures.

### BU Phase 1 -- Deterministic

1. Parse `git diff` to find changed functions (`ChangedFunction` with old/new bodies)
2. For each changed function, find associated test files
3. Diff test assertions between refs
4. Skip functions already flagged by TD (via `SharedFindings` broadcast channel)

### BU Phase 2 -- LLM Analysis

For functions not covered by TD:
1. Infer behavioral spec (`FunctionSpec`) from old body
2. Infer spec from new body (with test context if available)
3. Compare specs: Tier 1 structural comparison first, Tier 2 LLM fallback
4. If breaking: check upward propagation through call graph

### Cross-Pipeline Coordination (`crates/core/src/shared.rs`)

`SharedFindings<L>` provides:
- `DashMap<String, StructuralChange>` -- TD writes, BU reads
- `broadcast::Sender<String>` -- TD broadcasts qualified names of structural breaks
- `BuReceiver` -- BU drains broadcast into local skip set

This prevents BU from re-analyzing functions already caught by TD.

---

## Pipeline Selection Decision Tree

```
User runs analyze command
    |
    +-- --behavioral flag? --> BU pipeline (TD + LLM behavioral analysis)
    |
    +-- default ------------> SD pipeline (TD + source-level deterministic analysis)
```

The SD pipeline is preferred because:
1. Deterministic and reproducible
2. No LLM cost or latency
3. Catches DOM/CSS/ARIA/composition changes that LLM often misses
4. Produces actionable Konveyor rules with precise fix strategies
