# TypeScript Crate (`crates/ts/`)

Implements `Language` for TypeScript/JavaScript/React. This is the most feature-rich language implementation.

## File Map

```
crates/ts/src/
  lib.rs                    Re-exports
  language.rs               Language trait impl for TypeScript
  cli.rs                    TS-specific CLI args
  extensions.rs             TsAnalysisExtensions wrapper
  symbol_data.rs            TsSymbolData (rendered_components, css)
  report.rs                 build_report() -- groups changes, discovers child components
  extract/
    mod.rs                  OxcExtractor -- multi-pass .d.ts API extraction (~4100 lines)
  canon/
    mod.rs                  Type canonicalization (7 normalization rules, ~1600 lines)
  source_profile/
    mod.rs                  extract_profile() -- single-pass component source profile extraction
    prop_defaults.rs        Default value extraction from destructuring patterns
    prop_style.rs           Prop-to-CSS-class binding detection
    managed_attrs.rs        Prop-to-HTML-attribute override detection (AST dataflow)
    diff.rs                 diff_profiles() -- 17-aspect profile comparison
    bem.rs                  BEM structure extraction (block/element/modifier)
    children_slot.rs        {children} JSX slot position tracing
    clone_element.rs        cloneElement prop injection detection
    react_api.rs            Portal, context, forwardRef, memo detection
  sd_pipeline.rs            Source-level diff pipeline orchestrator
  sd_types.rs               All SD pipeline types (~18 structs/enums)
  composition/
    mod.rs                  build_composition_tree_v2() -- ~20-step evidence-based tree builder
  diff_parser/
    mod.rs                  Git diff -> ChangedFunction parser
  test_analyzer/
    mod.rs                  Test file discovery + assertion diff
  call_graph/
    mod.rs                  Same-file call graph (find callers/references)
  jsx_diff/
    mod.rs                  JSX render output differ (elements, ARIA, roles, CSS, data-*)
  css_scan/
    mod.rs                  CSS variable/class scanner (--pf-v5-*, pf-v5-c-*)
  css_profile/
    mod.rs                  CSS structure extraction (BEM, grid, flex, :has(), combinators)
  manifest/
    mod.rs                  package.json differ (entry points, exports, peer deps, engines)
  konveyor.rs               TD pipeline rule generation
  konveyor_v2.rs            SD pipeline rule generation
  konveyor_frontend.rs      Frontend rule helpers
  deprecated_replacements.rs Deprecated component detection (rendering swap + commit co-change)
  git_utils.rs              Shared git utilities
  resolve.rs                Import path resolution via oxc_resolver
  worktree/
    mod.rs                  Exports, RefBuildConfig
    guard.rs                WorktreeGuard RAII lifecycle
    error.rs                WorktreeError (16 variants with tips)
    tsc.rs                  TypeScript compilation (3 strategies)
    package_manager.rs      npm/yarn/pnpm detection
    nvm.rs                  Node.js version resolution via nvm
  llm_prompts.rs            React-specific LLM prompt builders
```

## Extraction (`extract/mod.rs`)

OXC-based multi-pass extraction from `.d.ts` files. This is the entry point for the TD pipeline.

### 7 Extraction Phases

**Phase -1 (Reachability filtering)**: Before any extraction, filter the set of `.d.ts` files down to those reachable from package entry points (`index.d.ts`). Uses `filter_to_reachable()` which traces `export * from` re-export chains. Unreachable files are excluded from all subsequent phases. Also builds a provenance map recording which entry point exports each file.

**Phase 0**: Scan `@types` packages for namespace declarations and `export as namespace` directives.

**Phase 1**: Collect imports from all `.d.ts` files (default, named, namespace, `/// <reference types>` directives). Build `ImportMap` for type resolution.

**Phase 2**: Extract symbols from each file using OXC AST walker. Handles: functions, classes, interfaces, type aliases, enums, variables, namespaces, default exports. Types are canonicalized via `canon::canonicalize_type_with_imports()`.

**Phase 3**: Set package names from `package.json` files found in the directory tree.

**Phase 4**: Set `import_path` via `set_import_paths()` using the provenance map built in Phase -1. Each symbol's import path is derived from which subpath export its file belongs to. This does not filter symbols (filtering happened in Phase -1).

**Phase 5**: Populate `rendered_components` and CSS tokens. For each `.d.ts` symbol, find the matching `.tsx` source file, parse its JSX, and extract: (a) rendered component names from JSX elements, (b) `styles.xxx` CSS token references.

### Dist Deduplication

When a package has multiple dist variants (ESM, CJS, etc.), the extractor keeps only the highest-priority one: `esm > mjs > es > js > cjs > commonjs > lib`. This prevents duplicate symbols from appearing in the API surface.

### Path Remapping

`remap_dist_to_src()` converts `dist/esm/components/Button/Button.d.ts` back to `src/components/Button/Button.tsx` for source-level references.

## Type Canonicalization (`canon/mod.rs`)

7 normalization rules ensure type comparisons are stable:

1. **Union/intersection sorting**: Members sorted alphabetically, nested unions flattened
2. **Array normalization**: `Array<T>` -> `T[]`, `ReadonlyArray<T>` -> `readonly T[]`
3. **Parenthesis cleanup**: Remove unnecessary parens, keep required ones (e.g., `(A | B)[]`)
4. **Whitespace normalization**: Consistent spacing in object types
5. **Type algebra**: `T | never` -> `T`, `T & unknown` -> `T`
6. **Import resolution**: `React.ReactNode`, `import("react").ReactNode`, and direct `ReactNode` all normalize to the same form. Uses `ImportMap` for resolution.
7. **Default generic parameters**: `ReactElement<any>` -> `ReactElement` (strips defaults)

## Source Profile Extraction (`source_profile/`)

`extract_profile()` performs a single-pass OXC AST walk of a `.tsx` source file, collecting ~25 fields into `ComponentSourceProfile`:

| Field | What It Captures |
|-------|-----------------|
| `rendered_elements` | HTML elements in JSX (`<div>`, `<button>`, etc.) with conditionality tracking |
| `rendered_components` | React components in JSX (`<Icon>`, `<Tooltip>`, etc.) |
| `aria_attributes` | ARIA attributes (`aria-label`, `aria-hidden`, etc.) |
| `role_attributes` | Role attributes (`role="button"`, etc.) |
| `data_attributes` | Data attributes (`data-testid`, etc.) |
| `prop_defaults` | Default values from destructuring (`{ size = 'md' }`) |
| `uses_portal`, `portal_target` | React portal usage |
| `consumed_contexts`, `provided_contexts` | React context dependencies |
| `is_forward_ref`, `is_memo` | React API wrappers |
| `css_tokens_used` | `styles.xxx` references |
| `bem_block`, `bem_elements`, `bem_modifiers` | BEM CSS structure |
| `prop_style_bindings` | Which props control which CSS classes |
| `extends_props` | Interface extends clauses |
| `children_slot_path` | Where `{children}` renders in the JSX tree |
| `all_props`, `required_props`, `prop_types` | Full prop interface |
| `deprecated_props` | Props with `@deprecated` JSDoc |
| `clone_element_injections` | `Children.map + cloneElement` patterns |
| `managed_attributes` | Prop-to-HTML-attribute overrides |
| `has_children_prop` | Whether the component accepts `children` at all |
| `children_slot_detail` | Enhanced children slot path with CSS token info per wrapper element |

### Profile Diffing (`source_profile/diff.rs`)

`diff_profiles()` compares old and new profiles across 17 dimensions, producing `SourceLevelChange` entries with categories like `DomStructure`, `AriaChange`, `CssToken`, `PropDefault`, `Composition`, etc.

## SD Pipeline (`sd_pipeline.rs`)

The pipeline orchestrator runs phases in an interleaved order (not grouped by letter). The execution order is:

**A** -> **B** -> **A.5** -> **B.5** -> **B.5b** -> **B.5c** -> **A.7a** -> **A.7b** -> **B1** -> **B3**

- **A**: For each changed component file, extract profiles at both refs and diff them.
- **B**: Extract source profiles for ALL components at the to-ref (full extraction, not just changed files). Does NOT build composition trees — that happens in B1.
- **A.5**: Deprecated migration diffing — diff deprecated components against their non-deprecated replacements (detected via `deprecated_replacements.rs`). Runs after B because it needs the full `new_profiles` map.
- **B.5**: Extends resolution — enrich `all_props` from inherited interfaces (`extends_props`). Needed before transitive analysis so delegating components have complete prop lists.
- **B.5b**: Enrich `overridden_attributes` from helper function source — resolve managed attribute bindings that delegate to external helper functions by parsing those functions' source.
- **B.5c**: Re-diff `PropAttributeOverride` with enriched profiles — Phase A emitted `PropAttributeOverride` changes before B.5b enrichment, so re-diff with the now-complete `overridden_attributes`.
- **A.7a**: Transitive behavioral change detection — detect changes in managed attribute helper functions that propagate to all components importing them. Runs after B.5 enrichment so `all_props` includes inherited props.
- **A.7b**: Transitive rendered-component change propagation — propagate source-level changes (DOM structure, ARIA, CSS token, etc.) through `rendered_components` chains.
- **B1**: Build composition trees per family (dependency-aware, ~20-step signal algorithm via `build_composition_tree_v2()`).
- **B3**: Composition diff + conformance checks — diff old vs new composition trees for changed families, generate conformance checks from all trees.

## Composition Tree Builder (`composition/mod.rs`)

The core algorithm for inferring React component hierarchy relationships.

### Input

- Map of component name -> `ComponentSourceProfile`
- Family export list (which components are public)
- CSS profiles (`CssBlockProfile` per BEM block)
- Delegate contexts (for wrapper families that wrap another family)

### ~20 Signal Steps

Each step can add edges or strengthen existing ones. Multiple signals for the same edge are combined via `EdgeStrength::combine()`.

| Step | Signal | Evidence Source |
|------|--------|----------------|
| 1 | Internal rendering | `rendered_components` (component X renders component Y) |
| 1.5 | Delegate projection | Wrapper family inherits delegate family edges |
| 2 | CSS direct-child | `.parent > .child` selectors |
| 3 | CSS grid | `grid-template` on parent, `grid-column` on child |
| 3b | Implicit grid children | Elements inside non-root grid containers that lack explicit `grid-column` |
| 3c | Re-parent through `display:contents` | Intermediaries with `display:contents` act as transparent wrappers; their children are re-parented to the actual layout ancestor |
| 4 | CSS flex | `display: flex` on parent |
| 5 | CSS descendant | `.parent .child` descendant selectors |
| 5.5 | CSS layout | Shared flex-wrap/gap rules |
| 6 | Context | Provider/consumer React context relationships |
| 7 | DOM nesting | HTML semantic nesting (`<ul>` -> `<li>`, `<table>` -> `<tr>`) |
| 8 | cloneElement | `Children.map + cloneElement` prop threading |
| 8.5 | BEM element orphan fallback | Connect orphan members whose BEM element matches the root's BEM block |
| 8.6 | Secondary BEM block sub-root | Re-run orphan logic using secondary BEM block owners as sub-roots |
| 8.7 | Prop-passed detection | Detect components passed via ReactNode/ReactElement props |
| 8.8 | Downgrade bidirectional CHP cycles | When two components each claim CHP on the other, downgrade the weaker edge to Allowed |
| 9 | Dedup | `deduplicate_edges()` — remove duplicate edges between the same pair |
| 9.5 | Pure composition wrapper PMC upgrade | Upgrade edges to wrappers that exist solely to compose children |
| 9.6 | Suppress root shortcuts | Remove root->leaf when root->mid->leaf exists (runs after 9.5 so Required wrappers are respected) |
| 10 | Prune disconnected | Remove members with no edges |

### EdgeStrength Collapse

`collapse_chain()` determines the effective strength of a transitive chain A -> B -> C, where `self` is the outer edge (A -> B) and `child_edge` is the inner edge (B -> C):
- **CHP** (C must be inside A) = `inner.CHP AND outer.PMC` — the child must need its intermediate parent AND the intermediate must be guaranteed present (parent always renders it)
- **PMC** (A must contain C) = `outer.PMC AND inner.PMC` — both links must say "parent requires child"
- Used in step 9.6 to decide whether to suppress direct root->leaf edges

## Konveyor Rule Generation

### TD Rules (`konveyor.rs`)

Generated from `AnalysisReport<TypeScript>` via `generate_rules()`. Covers structural API changes detected by the top-down pipeline:
- Component renamed/removed/relocated rules
- Prop removed/type-changed/renamed rules
- Signature changed rules
- Import path change rules
- Manifest change rules
- CSS variable prefix change rules

### SD Rules (`konveyor_v2.rs`)

Generated from `SdPipelineResult` via `generate_sd_rules()`. Contains 17 generator functions:

| Generator | What It Produces |
|-----------|-----------------|
| `generate_composition_change_rules` | New required children, prop-to-child migrations |
| `generate_conformance_rules` | `notParent`, `invalidDirectChild`, `requiresChild`, `exclusiveWrapper` |
| `generate_context_rules` | Context dependency changes |
| `generate_prop_child_migration_rules` | Props migrated to child component slots |
| `generate_cross_family_child_to_prop_rules` | Cross-family child-to-prop migrations |
| `generate_deprecated_migration_rules` | Deprecated-to-replacement migration guidance |
| `generate_prop_value_conformance_rules` | Prop value constraints from conformance checks |
| `generate_required_prop_added_rules` | Newly required props |
| `generate_test_impact_rules` | Test selector/assertion changes |
| `generate_portal_prop_rules` | Portal target prop changes |
| `generate_composition_inversion_rules` | Parent/child relationship inversions |
| `generate_prop_attribute_override_rules` | Managed HTML attribute override changes |
| `generate_deprecated_prop_rules` | Newly deprecated props |
| `generate_css_class_removal_rules` | Removed CSS BEM blocks |
| `generate_removed_css_file_rules` | Entirely removed CSS files |
| `generate_dead_css_class_rules` | CSS classes no longer referenced by any component |
| `generate_enumerated_css_class_rules` | Enumerated CSS class value changes |

## Deprecated Replacement Detection (`deprecated_replacements.rs`)

Two detection strategies for pairing deprecated components with their replacements:

1. **Rendering swap analysis**: Find host components that switched from rendering the old component to rendering the new one between refs
2. **Git commit co-change**: Find deprecation commits (adding files to `deprecated/components/`) and check which other component directories were modified in the same commit

Results feed into the SD pipeline for differential profile analysis.

## Worktree Lifecycle (`worktree/`)

RAII lifecycle for git worktrees:
1. Create detached worktree via `git worktree add`
2. Resolve Node.js version via nvm (if `--from-node-version` / `--to-node-version` specified)
3. Detect package manager from lockfile (`npm`, `yarn`, `pnpm`)
4. Install dependencies
5. Run `tsc --declaration` (3 strategies: root tsconfig, solution tsconfig with `--build`, per-package)
6. On drop: `git worktree remove`

`WorktreeError` has 16 variants, each with a user-facing tip via the `ErrorTip` trait.
