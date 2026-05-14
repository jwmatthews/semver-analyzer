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
    mod.rs                  OxcExtractor -- multi-pass .d.ts API extraction (~2000 lines)
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
  sd_types.rs               All SD pipeline types (~50 structs/enums)
  composition/
    mod.rs                  build_composition_tree_v2() -- 10-step evidence-based tree builder
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
    error.rs                WorktreeError (14 variants with tips)
    tsc.rs                  TypeScript compilation (3 strategies)
    package_manager.rs      npm/yarn/pnpm detection
    nvm.rs                  Node.js version resolution via nvm
  llm_prompts.rs            React-specific LLM prompt builders
```

## Extraction (`extract/mod.rs`)

OXC-based multi-pass extraction from `.d.ts` files. This is the entry point for the TD pipeline.

### 6 Extraction Phases

**Phase 0**: Scan `@types` packages for namespace declarations and `export as namespace` directives.

**Phase 1**: Collect imports from all `.d.ts` files (default, named, namespace, `/// <reference types>` directives). Build `ImportMap` for type resolution.

**Phase 2**: Extract symbols from each file using OXC AST walker. Handles: functions, classes, interfaces, type aliases, enums, variables, namespaces, default exports. Types are canonicalized via `canon::canonicalize_type_with_imports()`.

**Phase 3**: Set package names from `package.json` files found in the directory tree.

**Phase 4**: Set `import_path` from entry point provenance. BFS from `index.d.ts` through `export * from` re-export chains to determine which subpath export each symbol belongs to. Files unreachable from any entry point are filtered out (non-public API).

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

### Profile Diffing (`source_profile/diff.rs`)

`diff_profiles()` compares old and new profiles across 17 dimensions, producing `SourceLevelChange` entries with categories like `DomStructure`, `AriaChange`, `CssToken`, `PropDefault`, `Composition`, etc.

## Composition Tree Builder (`composition/mod.rs`)

The core algorithm for inferring React component hierarchy relationships.

### Input

- Map of component name -> `ComponentSourceProfile`
- Family export list (which components are public)
- CSS profiles (`CssBlockProfile` per BEM block)
- Delegate contexts (for wrapper families that wrap another family)

### 10 Signal Steps

Each step can add edges or strengthen existing ones. Multiple signals for the same edge are combined via `EdgeStrength::combine()`.

| Step | Signal | Evidence Source |
|------|--------|----------------|
| 1 | Internal rendering | `rendered_components` (component X renders component Y) |
| 1.5 | Delegate projection | Wrapper family inherits delegate family edges |
| 2 | CSS direct-child | `.parent > .child` selectors |
| 3 | CSS grid | `grid-template` on parent, `grid-column` on child |
| 4 | CSS flex | `display: flex` on parent |
| 5 | CSS descendant | `.parent .child` descendant selectors |
| 5.5 | CSS layout | Shared flex-wrap/gap rules |
| 6 | Context | Provider/consumer React context relationships |
| 7 | DOM nesting | HTML semantic nesting (`<ul>` -> `<li>`, `<table>` -> `<tr>`) |
| 8 | cloneElement | `Children.map + cloneElement` prop threading |
| 8.5-8.7 | BEM/prop cleanup | Orphan fallback, secondary blocks, prop-passed detection |
| 9 | Intermediate suppression | Remove root->leaf when root->mid->leaf exists |
| 10 | Prune disconnected | Remove members with no edges |

### EdgeStrength Collapse

`collapse_chain()` determines the effective strength of a chain A -> B -> C:
- If the intermediate edge (A -> B) is `Required` or `Structural`, the transitive edge (A -> C) inherits the child's CHP requirement
- Used in step 9 to decide whether to suppress direct edges

## Konveyor Rule Generation

### TD Rules (`konveyor.rs`)

Generated from `AnalysisReport<TypeScript>`:
- Component renamed/removed/relocated rules
- Prop removed/type-changed/renamed rules
- Signature changed rules
- Import path change rules
- Manifest change rules

### SD Rules (`konveyor_v2.rs`)

Generated from `SdPipelineResult`:
- Composition change rules (new required children, prop-to-child migrations)
- Conformance rules (notParent, invalidDirectChild, requiresChild, exclusiveWrapper)
- Context dependency rules
- Deprecated migration rules
- Required prop added rules
- Test impact rules
- Portal prop rules
- CSS token/class change rules
- Deprecated prop rules

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

`WorktreeError` has 14 variants, each with a user-facing tip via the `ErrorTip` trait.
