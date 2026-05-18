# Composition Tree V2 — Design & Verification

## Status

**Implemented** — V2 builder is wired into the SD pipeline. V1 builder is still
used for old-version tree building (composition diff) and existing tests.

## Problem

The v1 composition tree builder uses BEM token analysis to create parent-child
edges. BEM can identify that a component belongs to a family (same CSS block)
but cannot determine depth — all children end up flat under root. This produces
incorrect conformance rules that fire as false positive incidents.

## Design: Evidence-Based Tree Building

### Core Principle

**BEM determines family membership. CSS + React source determine hierarchy.**

Every edge in the tree must have structural evidence from one of 10 signals
(8 structural + 2 fallback). Components with zero edges are dropped from the
tree entirely — no "default to root" guessing.

### EdgeStrength: Four Variants (Two-Dimensional)

Every `CompositionEdge` has a `strength` field encoding two independent
constraint dimensions:

- **CHP (child-must-have-parent)**: Does the child break outside the parent?
- **PMC (parent-must-have-child)**: Does the parent need this child?

| Strength | CHP | PMC | Rule types generated |
|------------|-----|-----|------------------------------|
| `Required` | YES | YES | `notParent` + `requiresChild` |
| `Structural` | YES | NO | `notParent` only |
| `Wrapper` | NO | YES | `requiresChild` only |
| `Allowed` | NO | NO | Neither (regex inclusion only) |

- **`Required`** — Both directions enforced. Rendering breaks without this
  nesting AND parent is meaningless without the child. Generates both
  `notParent` and `requiresChild` conformance rules.
- **`Structural`** — Child must be inside this parent when used, but the parent
  can exist without the child. CSS `>`, CSS grid, React context, DOM nesting
  (non-container tags). Generates `notParent` rules only.
- **`Wrapper`** — Parent must contain this child, but the child works
  standalone. cloneElement with ReactElement children, unconditional internal
  rendering. Generates `requiresChild` rules only.
- **`Allowed`** — Valid placement documented in CSS descendant selectors or flex
  context heuristics, but not the only valid placement. Stays in the tree for
  migration guidance but produces zero conformance rules.

Strengths are combined via `EdgeStrength::combine()` which ORs each dimension
independently: if ANY signal says CHP=YES, the result has CHP=YES; if ANY
signal says PMC=YES, the result has PMC=YES. For example,
`Structural.combine(Wrapper) = Required`.

This distinction is critical: a CSS descendant selector `.toolbar .group` proves
that ToolbarGroup can appear somewhere inside Toolbar, but it doesn't prove
ToolbarGroup MUST be a direct child of Toolbar (it should be inside
ToolbarContent). The edge is `Allowed` — informational, no false conformance
rule.

### Signal Steps

The v2 builder (`build_composition_tree_v2` in `composition/mod.rs`) runs these
steps in order:

| Step | Signal | Strength | What It Detects |
|------|--------|----------|----------------|
| 1 | Internal rendering | `Wrapper` (unconditional) / `Allowed` (conditional) | Component A renders component B in its JSX (including prop-default JSX). Unconditional = always rendered (PMC=YES). Conditional = may not render (PMC=NO, CHP=NO). |
| 1.5 | Delegate tree projection | Inherited from delegate edge | Wrapper family inherits edges from delegate family's tree via `extends_props`. Each projected edge clones the delegate edge's strength (`edge.strength.clone()`). |
| 2 | CSS direct-child `>` | `Structural` (normal) / `Allowed` (recursive, ambiguous, or has-reverse) | `.block__A > .block__B` selector in CSS. Downgraded to `Allowed` when child=root (recursive nesting), parent or child is ambiguous (multiple components share CSS element), or reverse edge already exists. |
| 3 | CSS grid parent-child | `Structural` (normal) / `Allowed` (child=root or ambiguous) | A has `grid-template-*`, B has `grid-column`/`grid-row` |
| 3b | CSS implicit grid child | `Structural` (normal) / `Allowed` (child=root or ambiguous) | B is in same block as non-root grid container A, has no grid positioning |
| 3c | CSS display:contents re-parenting | `Structural` (grid-column-reverts) / `Allowed` (variable-child-refs) | Mode-switcher element (display:contents <-> flex) re-parents grid children with reverting grid-column and variable-ref children to the mode-switcher component instead of root |
| 4 | CSS flex context | Allowed | Root is grid, A wraps children in flex, B has no grid positioning |
| 5 | CSS descendant | Allowed | `.block__A .block__B` selector in CSS |
| 5.5 | CSS layout children | Allowed | Shared CSS rule with flex-wrap/gap implies containment |
| 6 | React context | `Structural` | A provides XContext, B consumes XContext (CHP=YES: child crashes without context, PMC=NO: parent can exist without consumers) |
| 7 | DOM nesting | Required | A wraps children in `<ul>`, B renders `<li>` |
| 8 | cloneElement | `Wrapper` (ReactElement children) / `Structural` (ReactNode children) | A uses `Children.map + cloneElement({ prop })`, B declares `prop`. ReactElement children: parent is purpose-built wrapper (PMC=YES, CHP=NO). ReactNode children: generic container (CHP=YES, PMC=NO). |
| 8.5 | BEM element orphan fallback | Allowed | Orphan BEM elements connected to root as last resort |
| 8.8 | Bidirectional CHP cycle downgrade | Downgrades weaker direction to `Allowed` | When A->B and B->A both have CHP=YES (Required or Structural), the weaker direction (lower EdgeStrength ordinal) is downgraded to `Allowed`. Equal strengths use incoming-edge-count tiebreaker. |
| 9.5 | Pure composition wrapper PMC upgrade | `Structural` -> `Required` (via combine with `Wrapper`) | Parents with no Internal edges AND structural evidence (CSS grid-template or pure container DOM tag like `<ul>`, `<table>`, `<dl>`) get PMC=YES added to their Structural DirectChild edges. |
| 9.6 | Suppress root shortcuts | Removes edge | Root->grandchild edges are removed when an intermediate path exists through a non-root parent. Runs after Step 9.5 so Required wrappers are available. |

After all steps: deduplicate (Step 9), suppress root shortcuts (Step 9.6),
drop members with zero edges (Step 10). Members with outgoing edges but no incoming
edges are retained as **secondary roots** — top-level containers within the
family (e.g., JumpLinksList wraps `<ul>` containing JumpLinksItem `<li>`
children, but nothing is above JumpLinksList in the hierarchy). Non-exported
secondary roots are then properly handled by `collapse_internal_nodes`:
since they have no incoming edges, collapsing removes their outgoing edges
cleanly with zero transitive edges created.

### CSS Layout Children (Step 5.5)

Step 5.5 consumes `layout_children` from `CssBlockProfile` — pairs of BEM
elements where one is a layout container (has `flex-wrap`, `gap`, or is a
grid container) and the other is a co-rule sibling. These pairs are mapped
through `css_element_to_component_map` to produce component edges.

This data was previously computed by `infer_layout_children()` in the CSS
profile module but never consumed by the composition tree builder. It catches
intermediate nesting within families (e.g., EmptyStateFooter → EmptyStateActions
from a shared CSS rule with flex-wrap).

### BEM Element Orphan Fallback (Step 8.5)

Step 8.5 connects orphan BEM elements to the family root as a last resort.
It fires for family members with zero incoming edges after all structural
signals (Steps 1-8 + 5.5) if the member appears in `css_element_to_component_map`
(has BEM element CSS tokens of the root's block) and the root has
`has_children_prop`. Three guards prevent false edges:

1. **Orphan gate**: Only fires for members with zero incoming edges, preventing
   wrong edges for already-connected components in Category 3 families.
2. **CSS element map membership**: Member must have CSS tokens that are BEM
   elements of the root's block (filters out context objects, type exports).
3. **BEM independence**: Member must NOT have its own distinct BEM block
   (prevents false edges for collision families like Label/LabelGroup,
   Menu/MenuToggle, Alert/AlertGroup where camelCase naming creates false
   prefix matches in the CSS element map).

This step recovers children-passthrough families (e.g., EmptyState, Panel,
HelperText, Sidebar) where the parent renders `{children}` and sub-components
are placed by consumers in JSX with no structural signal connecting them.

### Prop-Default JSX Detection (Step 1 enhancement)

Step 1 detects components rendered via `rendered_components` in the source
profile. The source profile extractor walks JSX elements in:

- Function/arrow body (return statements, variable initializers, conditionals)
- **Parameter destructuring defaults**: `({ bar = <Bar /> }) => { ... }`
- **Variable destructuring defaults**: `const { icon = <Icon /> } = this.props`

This is critical for components like ChartBullet that receive sub-components
as props with JSX defaults (e.g.,
`comparativeErrorMeasureComponent = <ChartBulletComparativeErrorMeasure />`).
Without this, Step 1 misses the rendering relationship and cloneElement
(Step 8) incorrectly creates a complete graph of peer edges.

### cloneElement Filtering (Step 8)

Step 8 has two filters to prevent false edges from shared prop vocabularies:

1. **Skip reverse-of-existing**: If B→A already exists from a prior step
   (e.g., Step 1 internal rendering), don't create A→B from cloneElement.
   The prior edge establishes the direction; adding the reverse creates a
   false cycle.

2. **Remove bidirectional pairs**: After creating all cloneElement edges,
   if both A→B and B→A exist from cloneElement, both are removed. This
   indicates a peer relationship (shared prop vocabulary) rather than a
   parent-child hierarchy. E.g., chart sub-components that all inject
   the same layout props (height, width, theme) into non-family primitives.

### CSS Element → Component Mapping

The `build_css_element_to_component_map` function maps CSS BEM element names
(e.g., `"content-section"`) to React components (e.g., `"ToolbarContent"`) via
`css_tokens_used`.

**Token format**: Tokens are stored as `"styles.drawerBody"` (with `"styles."`
prefix). The mapping strips `"styles."` and skips `"styles.modifiers."` before
matching against the BEM block name.

**Block prefix stripping**: Token `"styles.toolbarContentSection"` with block
`"toolbar"` → strip `"styles."` → `"toolbarContentSection"` → strip `"toolbar"`
→ `"ContentSection"` → lowercase first char → `"contentSection"` → kebab →
`"content-section"`.

**Ambiguity**: When multiple components render the same CSS element (e.g.,
`DrawerContentBody` and `DrawerPanelBody` both render `__body`), the first
component processed wins. Known issue — needs multi-component token map with
parent-context disambiguation.

### CSS Profile Loading

CSS profiles come from a dependency repo (e.g., `@patternfly/patternfly`).
The pipeline:

1. `WorktreeGuard::create_only` creates a git worktree (no tsconfig required)
2. Caller-provided build command runs in the worktree (e.g.,
   `yarn install && npx gulp buildPatternfly`)
3. `extract_css_profiles_from_dir` reads ALL `.css` files per component
   directory and merges them via `merge_css_profile`
4. CSS profiles are passed to `build_composition_tree_v2` as `Option<&CssBlockProfile>`

### Collapsed Edges

`collapse_internal_nodes` removes non-exported internal components and creates
transitive edges. The collapsed edge strength is computed with three-branch
logic (not simply "the stronger strength"):

1. **Outer is `Wrapper`** (PMC=YES, CHP=NO): The internal node is a
   passthrough for `{children}` — it doesn't make rendering decisions, it
   just wraps content. The transitive edge **inherits the inner edge's
   strength directly**. The internal node is transparent.

2. **Outer is `Allowed`** (PMC=NO, CHP=NO): The conditionality is real — the
   internal node may not be rendered at all. The transitive edge is **always
   `Allowed`** regardless of the inner edge's strength.

3. **Outer is `Structural` or `Required`**: Falls back to the standard
   `collapse_chain()` AND logic:
   - CHP = inner.CHP AND outer.PMC (child must be inside intermediate AND
     intermediate is guaranteed inside parent)
   - PMC = outer.PMC AND inner.PMC (both links require parent-has-child)

**Cycle detection**: The collapse loop tracks which internal nodes have been
processed in a `collapsed_set`. When creating a transitive edge, if the target
child is already in `collapsed_set`, the edge is skipped — it would re-enter
a cycle among internal nodes and never reach an exported surface. This breaks
cycles like `TreeViewList → TreeViewRoot → TreeViewListItem → TreeViewList`
cleanly in O(n) iterations instead of hitting the 100-iteration safety limit.

### 5 Technical Enforcement Mechanisms

Every required nesting has a concrete technical reason rendering breaks without
it:

1. **CSS layout contracts** — Child needs to be a direct child of a specific
   CSS layout context (grid or flex). Example: DescriptionListGroup is
   `display: grid`, Term/Description must be its grid children.

2. **React context** — Parent provides context that descendant consumes. Null
   context causes runtime crash. Example: ToolbarContent provides
   ToolbarContentContext consumed by ToolbarToggleGroup.

3. **cloneElement prop threading** — Parent uses `Children.map + cloneElement`
   to inject props children depend on. Example: DataListItem injects `rowid`
   consumed by DataListItemRow.

4. **CSS direct-child selectors** — CSS rules require exact parent-child DOM
   path. Example: Drawer has 40+ rules using `.drawer > .drawer__main > .drawer__panel`.

5. **DOM semantics** — HTML nesting rules (`<ul>` → `<li>`, `<dl>` → `<dt>`).
   Example: NavList renders `<ul>`, NavItem renders `<li>`.

---

## Verification Scorecard

Verified against upstream PatternFly v6 documentation for all 115 component
families (110 main + 5 deprecated).

### Current State (post-session-3 fixes)

| Metric | Count |
|--------|-------|
| Total composition trees | 115 (110 main + 5 deprecated) |
| Total edges | 206 (Required: 157, Allowed: 49) |
| Conformance checks generated | 88 |
| Non-member parent edges | 0 |
| Non-member child edges | 0 |
| Duplicate member entries | 0 |
| Families CORRECT | 76 |
| Families CORRECT (minor notes) | 6 |
| Families WRONG | 28 |

### Progress Over Sessions

| Metric | Start (BEM v1) | Session 1 (EdgeStrength) | Session 3 (current) |
|--------|---------------|--------------------------|---------------------|
| Total edges | ~300 | 261 | 206 |
| Conformance checks | ~600 | 488 | 88 |
| Non-member edges | ~50 | 39 | 0 |
| Duplicate members | ? | 9 | 0 |
| Correct families (of 110) | ~40% | ~70% | 75% |

### Session 3 Fixes Applied

1. **Secondary root retention** (Step 10): Components with outgoing edges but
   no incoming edges are kept as secondary roots. Non-exported ones are properly
   collapsed. Reduced non-member parent edges from 34 → 0.

2. **Prop-default JSX detection + cloneElement filtering**: Source profile
   extractor now detects JSX elements in parameter and variable destructuring
   defaults. Combined with bidirectional pair removal and reverse-of-existing
   skip in Step 8. Eliminated ChartBullet's 60 wrong edges and 408 wrong
   conformance rules → 10 correct edges, 0 wrong rules.

3. **Collapse cycle detection**: Tracks collapsed internal nodes to break
   cycles among internal components. Fixes TreeView infinite loop (100
   iterations → 3 iterations, clean exit).

4. **Recursive nesting = Allowed**: CSS edges where the child component equals
   the family root (recursive/self-nesting patterns like DataList inside
   DataListContent, Menu inside MenuItem) are marked `Allowed` instead of
   `Required`. The nesting is valid but optional.

5. **Family path modifier prefix**: `extract_family_from_path` now prefixes
   family names with `deprecated/` or `next/` when the component lives under
   a modifier directory. Fixes DualListSelector duplicate members (17 → 6
   unique in main, 7 in deprecated). Also separates deprecated Modal, Wizard,
   Table, Chip, DragDrop, and Tile into their own families.

6. **code-connect exclusion**: Files from the `code-connect` Figma integration
   package are excluded from SD file discovery.

### Remaining Issues by Category

#### Category 1: Missing sub-components (11 families)

Components documented in PF docs but not appearing in family member lists.
Root cause: components not in the same directory, not exported from the same
index, or no structural signals connecting them.

| Family | Missing members |
|--------|----------------|
| ActionList | ActionListGroup, ActionListItem |
| CodeBlock | CodeBlockAction, CodeBlockCode |
| EmptyState | EmptyStateBody, EmptyStateFooter, EmptyStateActions |
| HelperText | HelperTextItem |
| Panel | PanelMain, PanelMainBody, PanelHeader, PanelFooter |
| Sidebar | SidebarContent, SidebarPanel |
| Dropdown | DropdownGroup, DropdownItem |
| Modal | ModalHeader, ModalFooter (+ missing edges to ModalBody) |
| TextInputGroup | TextInputGroupUtilities |
| Hint | HintTitle |
| NotificationDrawer | NotificationDrawerBody, NotificationDrawerHeader, NotificationDrawerGroup, NotificationDrawerGroupList |

#### Category 2: CSS token mapping errors (2 families)

**RESOLVED — Recursive nesting edges are Allowed, not Required** (DataList,
Menu): CSS rules like `.expandable-content-body > .dataList` and
`.list-item > .menu` represent valid recursive nesting patterns. These edges
are now correctly marked `Allowed` (fix #4 above).

**Shared CSS element name** (Drawer): `DrawerContentBody` and `DrawerPanelBody`
both use `styles.drawerBody`. First-wins maps `"body"` to `DrawerContentBody`.
CSS rule `.panel > .body` creates `DrawerPanelContent -> DrawerContentBody`
instead of `DrawerPanelContent -> DrawerPanelBody`. Compounded by
`DrawerPanelBody` not being in the family members at all.

**CSS element collision** (DataList): Multiple components share
`styles.dataListItemAction` and `styles.dataListItemControl`. First-wins
assigns to wrong component in some contexts.

#### Category 3: Wrong parent-child relationships (12 families)

| Family | Wrong edge | Correct relationship |
|--------|-----------|---------------------|
| Alert | AlertGroup -> AlertActionCloseButton | Alert -> AlertActionCloseButton |
| DataList | DataListContent -> DataList (root) | RESOLVED — now Allowed (recursive nesting) |
| DescriptionList | Term -> Description | Siblings in Group, not parent-child |
| Drawer | DrawerPanelContent -> DrawerContentBody | Should be DrawerPanelBody (missing) |
| InputGroup | InputGroupText -> InputGroupItem | Reversed — Item wraps Text |
| LoginPage | LoginPage -> LoginMain* | Children of Login, not LoginPage |
| Masthead | Masthead -> MastheadBrand, MastheadContent -> Toggle/Logo | Brand/Toggle in MastheadMain, Logo in MastheadBrand |
| Nav | NavItem -> NavExpandable | Reversed — NavExpandable contains NavItem |
| OverflowMenu | Flat star under root | OverflowMenuContent as intermediate |
| Page | PageBreadcrumb as parent of many | PageBreadcrumb is a child, not parent |
| Select | SelectOption -> Select (root) | RESOLVED — now Allowed (projected recursive nesting) |
| Tabs | Tabs -> TabTitleText | Should be Tab -> TabTitleText |

#### Category 4: Structural/data issues (3 families)

| Family | Issue |
|--------|-------|
| DualListSelector | RESOLVED — deprecated/main separation (fix #5 above) |
| Menu | Missing MenuGroup, MenuSearch, MenuSearchInput, MenuContainer |
| Wizard | WizardContext is a context object, not a component. Missing WizardHeader, WizardFooterWrapper. |
| SimpleList | Missing SimpleList -> SimpleListItem and SimpleList -> SimpleListGroup edges |

#### Category 5: Rule generation direction (future work)

Secondary roots (LabelGroup, AlertGroup, JumpLinksList, etc.) have edges TO
the primary root component. These edges are structurally correct but generate
the wrong type of conformance rule. Current: "Label must be inside LabelGroup"
(wrong — Label can be standalone). Correct: "If LabelGroup exists, it must
contain Labels" (constraint on the parent, not the child). This requires a new
`requiresChildren` rule type in `konveyor_v2.rs`, not a tree structure change.

---

## Key Files

| File | Purpose |
|------|---------|
| `crates/ts/src/sd_types.rs` | `CompositionEdge`, `EdgeStrength`, `CompositionTree` types |
| `crates/ts/src/composition/mod.rs` | `build_composition_tree_v2` — the v2 builder |
| `crates/ts/src/css_profile/mod.rs` | CSS profile extraction, `CssBlockProfile`, `CssElementInfo` |
| `crates/ts/src/source_profile/mod.rs` | Source profile extraction (JSX walk, cloneElement detection, prop-default JSX) |
| `crates/ts/src/source_profile/children_slot.rs` | `trace_children_slot_both` — children path + CSS token detail |
| `crates/ts/src/source_profile/clone_element.rs` | `detect_clone_element_injections`, `try_extract_clone_element_from_call` |
| `crates/ts/src/sd_pipeline.rs` | Pipeline orchestration, `collapse_internal_nodes` (with cycle detection) |
| `crates/ts/src/konveyor_v2.rs` | Conformance rule generation (filters by Required) |
| `src/orchestrator.rs` | Dep-repo worktree creation (`create_only` + build command) |
