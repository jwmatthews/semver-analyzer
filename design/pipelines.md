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
- `symbol`, `qualified_name`, `kind`, `package`, `before`, `after`, `description`, `is_breaking`, `impact: Option<ImpactAnalysis>`, `migration_target`

---

## SD Pipeline (Source-Level Diff, Deterministic)

**Default pipeline. Source: `crates/ts/src/sd_pipeline.rs` (TS), `crates/java/src/sd_pipeline.rs` (Java)**

Analyzes source code changes at the AST level without LLM. Produces deterministic, reproducible results.

### TypeScript SD Pipeline

Phases execute in interleaved order (not grouped by letter):

**A** -> **B** -> **A.5** -> **B.5** -> **B.5b** -> **B.5c** -> **A.7a** -> **A.7b** -> **B1** -> **B3**

**Phase A -- Changed File Analysis**
1. Find changed `.tsx` files via `git diff --name-only`
2. For each changed file, extract `ComponentSourceProfile` at both refs (25+ fields: rendered elements, ARIA attributes, prop defaults, CSS tokens, BEM structure, portal usage, contexts, etc.)
3. Diff profiles to produce `SourceLevelChange` entries across 15 categories

**Phase B -- Full To-Version Extraction** (lines ~134-186)
1. Enumerate ALL component files at the to-ref
2. Extract source profiles for every component into `new_profiles`
3. Handle deprecated/non-deprecated name collisions (main path wins)

**Phase A.5 -- Deprecated Migration Diffing**
- Runs after B because it needs the full `new_profiles` map
- For deprecated components removed in the new version, finds same-named non-deprecated replacements
- Diffs the deprecated component's old profile against the replacement's new profile
- Tags resulting changes with `migration_from` to distinguish from same-component evolution

**Phase B.5 -- Extends Resolution**
- Resolves `extends_props` entries (e.g., `extends OUIAProps`) to actual prop lists by following imports and parsing extended interfaces
- Enriches `all_props` for both old and new profiles
- Required before Phase A.7 because managed attribute detection uses `all_props` as `known_props`

**Phase B.5b -- Overridden Attributes Enrichment**
- For `ManagedAttributeBinding` entries with empty `overridden_attributes`, resolves the generator function's import
- Parses the function's return value to fill in the attribute names it produces at runtime
- Needed for helpers like `getOUIAProps` that generate attributes (`data-ouia-component-type`, etc.) not statically visible in JSX

**Phase B.5c -- Re-diff PropAttributeOverride**
- Phase A emitted `PropAttributeOverride` changes before B.5b enrichment, so their `overridden_attributes` were empty
- Re-diffs managed attributes with the enriched profiles to produce changes with real attribute names
- Replaces the stale Phase A entries with corrected versions

**Phase A.7a -- Transitive Managed Attribute Dependencies**
- Parses changed functions via `git diff` (runs inside SD, not in orchestrator)
- For each changed helper function used as a `generator_function` in `ManagedAttributeBinding`, generates transitive behavioral change entries
- Detects when helpers like `getOUIAProps`/`useOUIAProps` change behavior affecting all consuming components

**Phase A.7b -- Rendered Component Propagation**
- Propagates externally-observable source-level changes through the `rendered_components` graph
- When a sub-component changes (portal behavior, DOM structure, ARIA roles, etc.), all parent components that render it inherit those effects

**Phase B1 -- Composition Tree Building** (lines ~402-672)
- Builds composition trees per family using `build_composition_tree_v2()` (~20-step signal algorithm)
- Dependency-aware: families with `extends_props` to another family are deferred until the delegate family's tree is built
- Two-phase resolution: independent families first, then deferred families in topological order

**Phase B3 -- Composition Diff + Conformance** (lines ~674-729)
- Diff old vs new composition trees for changed families
- Generate conformance checks from all to-version trees

**Composition Tree Builder** (`crates/ts/src/composition/mod.rs`):
~20 signal steps combine evidence to build parent-child edges:
1. Internal rendering (from `rendered_components` in JSX)
1.5. Delegate tree projection
2. CSS direct-child selectors (`.parent > .child`)
3. CSS grid parent-child (grid-template vs grid-column)
3b. Implicit grid children
3c. Re-parent through `display:contents`
4. CSS flex context (flex container -> flex items)
5. CSS descendant selectors (`.parent .child`)
5.5. CSS layout children
6. React context (provider/consumer)
7. DOM nesting (`<ul>` -> `<li>`)
8. cloneElement threading
8.5. BEM element orphan fallback
8.6. Secondary BEM block sub-root
8.7. Prop-passed detection
8.8. Downgrade bidirectional CHP cycles
9. Dedup edges
9.5. Pure composition wrapper PMC upgrade
9.6. Suppress root shortcuts
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

**Phase A -- Changed File Diff**
- Find changed `.java` files via `git diff --name-only --diff-filter=AMRC` (excludes test files, `package-info.java`)
- Extract `JavaClassProfile` at both refs using tree-sitter
- Diff old vs new profiles to produce `JavaSourceChange` entries

**Phase B -- Full Extraction**
- Extract all `JavaClassProfile`s at the to-ref (from worktree if available, else via `git show`)

**Phase B.5 -- Inheritance Resolution**
- Resolve inheritance chains to detect transitive `Serializable` implementation
- For changed classes that are serializable, diff serialization-specific fields (serial version UID, serializable field changes)

**Phase B1 -- Inheritance Summary**
- Build inheritance trees from all new-version profiles
- Detect hierarchy breakages (changed superclass, removed interface implementation)

**Phase B3 -- Module System Diff**
- Diff `module-info.java` directives between old and new versions
- Detects added/removed exports, requires, opens directives

23 source-level change categories including: annotation changes, synchronization, exceptions, serialization, override, constructor dependencies, module exports, final/sealed, inheritance, native.

### Output

TypeScript: `SdPipelineResult` with ~20 fields including `source_level_changes`, `composition_trees`, `composition_changes`, `conformance_checks`, component props/types inventories, CSS inventories, deprecated replacements.

Java: `JavaSdPipelineResult` with `source_level_changes`, profiles, module changes, inheritance summary.

---

## Extended Analysis Phase (Post-SD, Pre-Report)

**Runs after SD completes, in the orchestrator. Source: `src/orchestrator.rs` (calls `Language::finalize_extensions()`)**

After both TD and SD finish, the orchestrator runs cross-pipeline post-processing before building the report. For TypeScript, this is implemented in `crates/ts/src/language.rs` (`finalize_extensions`) and `crates/ts/src/deprecated_replacements.rs`:

1. **Deprecated replacement detection (rendering swap)** -- Primary method. Examines SD composition data to find deprecated components whose host component started rendering a differently-named replacement component (e.g., deprecated `ApplicationLauncher` replaced by `Dropdown`)
2. **Deprecated replacement detection (commit co-change)** -- Fallback for components not detected by rendering swap. Analyzes git commit history to find deprecated components that were added in the same commit as their replacement
3. **Deprecated migration diffing for renamed replacements** -- Diffs renamed deprecated components against their replacements (e.g., `ChipGroup` vs `LabelGroup`). Phase A.5 only handles same-name lookups; this covers cross-name renames
4. **Structural change transformation** -- Rewrites TD structural changes based on detected deprecated replacements (e.g., marks a removal as "replaced by X" instead of plain removal)

---

## BU Pipeline (Bottom-Up, Behavioral)

**Opt-in via `--behavioral`. Source: `src/orchestrator.rs` + `crates/llm/`**

Uses LLM to detect behavioral changes not visible from type signatures.

### BU Phase 1 -- Deterministic

1. Parse `git diff` to find changed functions (`ChangedFunction` with old/new bodies)
2. For each changed function, find associated test files
3. Diff test assertions between refs
4. Skip functions already flagged by TD (via `SharedFindings` broadcast channel)
5. **Deterministic body analysis**: Delegates to `Language::body_analyzer()` (e.g., JSX diff + CSS scan for TypeScript). For each exported changed function with old/new bodies, runs language-specific AST analysis to detect behavioral changes without LLM (DOM structure changes, CSS class changes, ARIA attribute changes, etc.)

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
