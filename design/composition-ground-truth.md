# Composition Tree & Edge Ground Truth (PatternFly v6.4.1)

This file contains verification datasets for composition tree correctness,
extracted from AGENTS.md. It is the definitive reference for validating
composition trees and conformance rules against upstream PF6 documentation.

**This is reference data, not agent instructions.** For the rules and
invariants governing composition tree construction, see AGENTS.md.

---

## Single-Component Families (Skip for Composition)

The following 50 families are genuinely single-component — one file in the
directory, no sub-components, no composition tree needed. Skip these during
composition tree validation:

Avatar, BackToTop, Backdrop, BackgroundImage, Badge, Banner, Brand, Button,
CalendarMonth, Chart, ChartArea, ChartAxis, ChartBar, ChartBoxPlot,
ChartContainer, ChartCursorContainer, ChartDonut, ChartGroup, ChartLabel,
ChartLegend, ChartLine, ChartPie, ChartPoint, ChartScatter, ChartStack,
ChartThreshold, ChartTooltip, ChartVoronoiContainer, Charts, Checkbox,
Content, DatePicker, Divider, FormControl, Icon, Line, NotificationBadge,
NumberInput, Radio, Sankey, Skeleton, SkipToContent, Spinner, Switch,
TextArea, TextInput, Timestamp, Title, Truncate, deprecated/Tile.

---

## Composition Tree Ground Truth

The expected consumer-facing API for each multi-component family is derived
from the barrel file (index.ts) exports at the v6.4.1 tag. Context providers
and type exports are excluded. This is the definitive reference for
composition tree validation.

Families not listed here are either single-component (see above) or
internally-rendered-only (Popover, Tooltip, AboutModal, SearchInput, Slider,
TimePicker, ChartBullet, ChartCursorTooltip, ChartLegendTooltip — all
sub-components are internally rendered, not consumer-placed).

| Family | Expected Exports | Notes |
|--------|-----------------|-------|
| Accordion | Accordion, AccordionContent, AccordionExpandableContentBody, AccordionItem, AccordionToggle | — |
| ActionList | ActionList, ActionListGroup, ActionListItem | — |
| Alert | Alert, AlertActionCloseButton, AlertActionLink, AlertGroup | AlertActionLink: prop-passed via `actionLinks` |
| Breadcrumb | Breadcrumb, BreadcrumbHeading, BreadcrumbItem | — |
| Card | Card, CardBody, CardExpandableContent, CardFooter, CardHeader, CardTitle | — |
| ClipboardCopy | ClipboardCopy, ClipboardCopyAction, ClipboardCopyButton | — |
| CodeBlock | CodeBlock, CodeBlockAction, CodeBlockCode | CodeBlockAction: prop-passed via `actions` |
| CodeEditor | CodeEditor, CodeEditorControl | — |
| DataList | DataList, DataListAction, DataListCell, DataListCheck, DataListContent, DataListControl, DataListDragButton, DataListItem, DataListItemCells, DataListItemRow, DataListText, DataListToggle | — |
| DescriptionList | DescriptionList, DescriptionListDescription, DescriptionListGroup, DescriptionListTerm, DescriptionListTermHelpText, DescriptionListTermHelpTextButton | — |
| Drawer | Drawer, DrawerActions, DrawerCloseButton, DrawerContent, DrawerContentBody, DrawerHead, DrawerPanelBody, DrawerPanelContent, DrawerPanelDescription, DrawerSection | DrawerPanelBody: multi-component CSS map |
| Dropdown | Dropdown, DropdownGroup, DropdownItem, DropdownList | — |
| DualListSelector | DualListSelector, DualListSelectorControl, DualListSelectorControlsWrapper, DualListSelectorList, DualListSelectorListItem, DualListSelectorPane, DualListSelectorTree | — |
| EmptyState | EmptyState, EmptyStateActions, EmptyStateBody, EmptyStateFooter | — |
| ExpandableSection | ExpandableSection, ExpandableSectionToggle | ExpandableSectionToggle: prop-passed via `toggleContent` |
| FileUpload | FileUpload, FileUploadField, FileUploadHelperText | — |
| Form | ActionGroup, Form, FormAlert, FormFieldGroup, FormFieldGroupExpandable, FormFieldGroupHeader, FormGroup, FormGroupLabelHelp, FormHelperText, FormSection | FormGroupLabelHelp: prop-passed via `label` |
| FormSelect | FormSelect, FormSelectOption, FormSelectOptionGroup | — |
| HelperText | HelperText, HelperTextItem | — |
| Hint | Hint, HintBody, HintFooter, HintTitle | — |
| InputGroup | InputGroup, InputGroupItem, InputGroupText | — |
| JumpLinks | JumpLinks, JumpLinksItem, JumpLinksList | — |
| Label | Label, LabelGroup | — |
| List | List, ListItem | — |
| LoginPage | Login, LoginFooter, LoginFooterItem, LoginForm, LoginHeader, LoginMainBody, LoginMainFooter, LoginMainFooterBandItem, LoginMainFooterLinksItem, LoginMainHeader, LoginPage | LoginFooterItem: prop-passed via `footer`; LoginForm: exported orphan (convenience composite) |
| Masthead | Masthead, MastheadBrand, MastheadContent, MastheadLogo, MastheadMain, MastheadToggle | — |
| Menu | DrilldownMenu, Menu, MenuBreadcrumb, MenuContainer, MenuContent, MenuFooter, MenuGroup, MenuItem, MenuItemAction, MenuList, MenuSearch, MenuSearchInput | MenuContainer: exported orphan (standalone orchestrator) |
| MenuToggle | MenuToggle, MenuToggleAction, MenuToggleCheckbox | MenuToggleCheckbox: exported orphan (opaque slot via `splitButtonItems`) |
| Modal | Modal, ModalBody, ModalFooter, ModalHeader | Cross-block: ModalBody/ModalFooter via Step 8.6 (modalBox sub-block) |
| MultipleFileUpload | MultipleFileUpload, MultipleFileUploadMain, MultipleFileUploadStatus, MultipleFileUploadStatusItem | — |
| Nav | Nav, NavExpandable, NavGroup, NavItem, NavItemSeparator, NavList | — |
| NotificationDrawer | NotificationDrawer, NotificationDrawerBody, NotificationDrawerGroup, NotificationDrawerGroupList, NotificationDrawerHeader, NotificationDrawerList, NotificationDrawerListItem, NotificationDrawerListItemBody, NotificationDrawerListItemHeader | — |
| OverflowMenu | OverflowMenu, OverflowMenuContent, OverflowMenuControl, OverflowMenuDropdownItem, OverflowMenuGroup, OverflowMenuItem | — |
| Page | Page, PageBody, PageBreadcrumb, PageGroup, PageSection, PageSidebar, PageSidebarBody, PageToggleButton | PageSidebar: prop-passed via `sidebar`; PageBreadcrumb: prop-passed via `breadcrumb` |
| Pagination | Pagination, ToggleTemplate | — |
| Panel | Panel, PanelFooter, PanelHeader, PanelMain, PanelMainBody | — |
| Progress | Progress, ProgressBar, ProgressContainer | — |
| ProgressStepper | ProgressStepper, ProgressStep | — |
| Select | Select, SelectGroup, SelectList, SelectOption | — |
| Sidebar | Sidebar, SidebarContent, SidebarPanel | — |
| SimpleList | SimpleList, SimpleListGroup, SimpleListItem | — |
| Table | (see note) | (see note) |
| Tabs | Tab, TabAction, TabContent, TabContentBody, TabTitleIcon, TabTitleText, Tabs | TabContentBody: cross-block via Step 8.6 (tabContent sub-block) |
| TextInputGroup | TextInputGroup, TextInputGroupMain, TextInputGroupUtilities | — |
| ToggleGroup | ToggleGroup, ToggleGroupItem | — |
| Toolbar | Toolbar, ToolbarContent, ToolbarExpandableContent, ToolbarExpandIconWrapper, ToolbarFilter, ToolbarGroup, ToolbarItem, ToolbarToggleGroup | — |
| TreeView | TreeView, TreeViewSearch | — |
| Wizard | Wizard, WizardBody, WizardFooter, WizardHeader, WizardNav, WizardNavItem, WizardStep, WizardToggle | WizardHeader: prop-passed via `header` |
| deprecated/Chip | Chip, ChipGroup | — |
| deprecated/DragDrop | DragDrop, Draggable, Droppable | DroppableContext: context export noise |
| deprecated/DualListSelector | DualListSelector, DualListSelectorControl, DualListSelectorControlsWrapper, DualListSelectorList, DualListSelectorListItem, DualListSelectorPane, DualListSelectorTree | — |
| deprecated/Modal | Modal, ModalBox, ModalBoxBody, ModalBoxCloseButton, ModalBoxFooter, ModalBoxHeader, ModalContent | — |
| deprecated/Table | Table, Body, Header | Deprecated legacy table API |
| deprecated/Wizard | Wizard, WizardBody, WizardFooter, WizardHeader, WizardNav, WizardNavItem, WizardToggle | WizardFooter: prop-passed via `footer` |

**Note on Table:** The Table family exports many components (Caption, Tbody,
Thead, Tr, Td, Th, etc.) plus utility wrappers (ActionsColumn, RowWrapper,
TreeRowWrapper, InnerScrollContainer, OuterScrollContainer, SelectColumn,
etc.). The current tree has 16 members and 18 edges. Some utility wrappers
appear as exported orphans (EditableSelectInputCell, EditableTextCell,
SelectColumn, FavoritesCell, OuterScrollContainer, InnerScrollContainer,
TableTypes).

**Summary:** All expected consumer-facing components are present in
composition trees. 3 components are retained as exported orphans with no
edges (LoginForm, MenuContainer, MenuToggleCheckbox) — these are
convenience composites or orchestrators with no structural composition
signal. Context providers (AlertContext, FormContext, TabsContext,
WizardContext, etc.) appear as orphan members when exported from barrel
files; they don't affect rule generation.

---

## Edge Ground Truth

The following table classifies every non-internal Required edge by the
two constraint dimensions (CHP = child-must-have-parent, PMC =
parent-must-have-child), verified against upstream PF6 documentation at
v6.4.1. This is the definitive reference for conformance rule correctness.

### Category A: Both Required (CHP=YES, PMC=YES) — 33 edges

Both `notParent` and `requiresChild` rules are valid for these edges.
All edges verified as `required` strength in the actual composition tree.
Edges marked with + were originally Cat B or Cat D but are acceptable as
Required because the `requiresChild` rule uses OR semantics ("parent must
contain at least one of these children"). The parent being empty is the
real defect; which specific child satisfies it is flexible.

| Family | Parent | Child | Signal |
|--------|--------|-------|--------|
| DataList | DataListItemCells | DataListCell | CSS `>` + purpose |
| DescriptionList | DescriptionList | DescriptionListGroup | DOM `<dl>` nesting + CSS grid |
| Drawer | DrawerHead | DrawerActions | CSS grid parent-child |
| FormSelect | FormSelect | FormSelectOptionGroup + | DOM `<select>` nesting; select must contain options or optgroups |
| FormSelect | FormSelectOptionGroup | FormSelectOption + | DOM `<optgroup>` nesting; optgroup must contain options |
| Hint | Hint | HintBody + | CSS grid; hint must contain body or footer |
| Hint | Hint | HintFooter + | CSS grid; same OR group as HintBody |
| JumpLinks | JumpLinksList | JumpLinksItem | Scroll spy context + list purpose |
| Masthead | Masthead | MastheadBrand | CSS grid; docs explicitly required |
| Masthead | Masthead | MastheadContent | CSS grid; docs explicitly required |
| Masthead | Masthead | MastheadMain | CSS grid; docs explicitly required |
| MultipleFileUpload | MultipleFileUploadStatus | MultipleFileUploadStatusItem | CSS + status list purpose |
| List | List | ListItem | DOM `<li>` in `<ul>` + list purpose |
| Nav | NavList | NavItem | DOM `<li>` in `<ul>` + list purpose |
| Nav | NavList | NavItemSeparator + | DOM `<li>` in `<ul>`; same OR group as NavItem |
| Nav | NavList | NavExpandable + | DOM `<li>` in `<ul>`; same OR group as NavItem |
| Nav | NavGroup | NavItem | Context + group purpose |
| Nav | NavGroup | NavItemSeparator + | DOM `<li>` in `<ul>`; same OR group as NavItem |
| Nav | NavGroup | NavExpandable + | DOM `<li>` in `<ul>`; same OR group as NavItem |
| Nav | NavExpandable | NavItemSeparator + | DOM `<li>` in `<ul>`; same OR group as NavItem |
| NotificationDrawer | NotificationDrawerList | NotificationDrawerListItem | CSS + list purpose |
| NotificationDrawer | NotificationDrawerListItem | NotificationDrawerListItemBody + | CSS grid; list item must contain header or body |
| ProgressStepper | ProgressStepper | ProgressStep | DOM `<li>` in `<ol>` + stepper purpose |
| SimpleList | SimpleListGroup | SimpleListItem | CSS + selection context |
| Table | Tbody | Tr | DOM `<tr>` in `<tbody>` |
| Table | Table | Tbody | DOM nesting |
| Table | Table | Thead | DOM nesting + a11y |
| Table | Table | Caption + | DOM `<caption>` in `<table>`; same OR group as Tbody/Thead |
| Table | Tr | Th | DOM `<th>` in `<tr>` |
| Table | Tr | Td | DOM `<td>` in `<tr>` |
| Table | Tr | ExpandableRowContent + | CSS `>` in `<tr>`; same OR group as Th/Td |
| Wizard | WizardNav | WizardNavItem | CSS + nav purpose |
| deprecated/Wizard | WizardNav | WizardNavItem | Same as v6 |

### Category B: CHP-only (CHP=YES, PMC=NO) — 32 edges

Only `notParent` is valid. `requiresChild` is **wrong** for these edges.
The child must be inside the parent IF used, but the parent does NOT
require the child.

| Family | Parent | Child | Signal | Why PMC=NO |
|--------|--------|-------|--------|------------|
| Alert | AlertGroup | Alert | DOM context | Alert is standalone; AlertGroup can be empty (toast/dynamic) |
| Card | Card | CardHeader | CSS `>` | PF docs: "may omit these components" |
| Card | Card | CardTitle | CSS `>` | PF docs: "may omit these components" |
| Card | Card | CardBody | CSS `>` | PF docs: "recommended" but not required |
| Card | Card | CardFooter | CSS `>` | PF docs: "may omit these components" |
| ChartBullet | ChartBullet | ChartBulletComparativeErrorMeasure | Prop-passed | Internally rendered by default; prop is optional customization |
| ChartBullet | ChartBullet | ChartBulletComparativeWarningMeasure | Prop-passed | Same |
| ChartBullet | ChartBullet | ChartBulletGroupTitle | Prop-passed | Same |
| ChartBullet | ChartBullet | ChartBulletPrimaryDotMeasure | Prop-passed | Same |
| ChartBullet | ChartBullet | ChartBulletPrimarySegmentedMeasure | Prop-passed | Same |
| ChartBullet | ChartBullet | ChartBulletQualitativeRange | Prop-passed | Same |
| ChartBullet | ChartBullet | ChartBulletTitle | Prop-passed | Same |
| DataList | DataListItem | DataListItemCells | cloneElement + CSS `>` | Cells are common but not sole child type |
| DataList | DataListItem | DataListItemRow | cloneElement + CSS `>` | Row is primary child but item has other children |
| DataList | DataListItem | DataListToggle | CSS context | Only for expandable items |
| DataList | DataListItemRow | DataListItemCells | cloneElement + CSS `>` | Row can have just controls |
| DataList | DataListItemRow | DataListToggle | CSS context | Only for expandable items |
| DescriptionList | DescriptionListGroup | DescriptionListDescription | CSS implicit grid | Group doesn't require Description to be present |
| Drawer | DrawerContent | DrawerPanelContent | Internal rendering | Content wraps panel but doesn't require it |
| Drawer | Drawer | DrawerContent | Internal rendering | Drawer has content by default internally |
| DualListSelector | DualListSelectorPane | DualListSelectorListItem | CSS signals | CSS insufficient for PMC |
| DualListSelector | DualListSelectorPane | DualListSelectorTree | CSS | Tree is alternative to List |
| Menu | MenuItem | MenuItemAction | Prop-passed (`actions`) | Actions are optional on MenuItem |
| Modal | Modal | ModalBody | Step 8.6 (cross-block BEM) | PF docs: "ModalBody...are not required" |
| Modal | Modal | ModalHeader | Step 8.6 (cross-block BEM) | PF docs: "ModalHeader...are not required" |
| Modal | Modal | ModalFooter | Step 8.6 (cross-block BEM) | PF docs: "ModalFooter...are not required" |
| Page | Page | PageBreadcrumb | Prop-passed | Breadcrumb is optional |
| Progress | Progress | ProgressBar | Internal rendering | ProgressBar is internally rendered |
| Table | Td | ExpandableRowContent | CSS | Only for expandable rows |
| Tabs | Tabs | Tab | Context only | Step 9.5 PMC upgrade doesn't fire; should be Cat A |
| Tabs | Tab | TabAction | Prop-passed (`actions`) | Actions are optional on Tab |
| ToggleGroup | ToggleGroup | ToggleGroupItem | CSS layout | Empty group is valid DOM |

### Category C: PMC-only (CHP=NO, PMC=YES) — 1 edge

Only `requiresChild` is valid. `notParent` is wrong.

| Family | Parent | Child | Signal | Why CHP=NO |
|--------|--------|-------|--------|------------|
| ChartDonutUtilization | ChartDonutThreshold | ChartDonutUtilization | JSX children | ChartDonutUtilization works standalone |

### Category D: Both Allowed (CHP=NO, PMC=NO) — 12 edges

These edges have `allowed` strength in the tree. No conformance rules
are generated. Some represent tree accuracy gaps where the edge SHOULD
be stronger but CSS/context signals are insufficient.

| Family | Parent | Child | Issue |
|--------|--------|-------|-------|
| Alert | AlertGroup | AlertActionCloseButton | WRONG PARENT: goes inside Alert via `actionClose` prop, not AlertGroup |
| Drawer | Drawer | DrawerPanelContent | CSS descendant only; no `>` or grid signal for CHP/PMC |
| DualListSelector | DualListSelectorTree | DualListSelectorControl | CSS/context signals insufficient for Required |
| DualListSelector | DualListSelectorTree | DualListSelectorPane | Same |
| DualListSelector | DualListSelectorPane | DualListSelectorControl | Same |
| TreeView | TreeView | TreeViewSearch | Passed via `toolbar` prop; most examples omit it |
| deprecated/DualListSelector | DualListSelectorPane | DualListSelectorListItem | CSS signals insufficient in deprecated family |
| deprecated/DualListSelector | DualListSelectorTree | DualListSelectorContext | Context dependency not detected |
| deprecated/DualListSelector | DualListSelectorTree | DualListSelectorControl | Same |
| deprecated/DualListSelector | DualListSelectorTree | DualListSelectorPane | Same |
| deprecated/DualListSelector | DualListSelectorPane | DualListSelectorContext | Same |
| deprecated/DualListSelector | DualListSelectorPane | DualListSelectorControl | Same |

### Edge Ground Truth Summary

| Category | Count | % | Current Status |
|----------|-------|---|----------------|
| A: Both Required (correct) | 33 | 42% | Correct — both rules valid |
| B: CHP-only | 32 | 41% | `notParent` valid; `requiresChild` wrong if generated |
| C: PMC-only | 1 | 1% | `requiresChild` valid; `notParent` wrong |
| D: Both Allowed | 12 | 15% | No conformance rules generated |
| **Total** | **78** | | |

---

## Known Tree / Conformance Rule Issues

The following edges and conformance rules have been analyzed against
upstream PF6 documentation at v6.4.1. Items marked with check are correct
despite appearing suspicious. Items marked with warning are genuine issues.

### Self-referencing invalidDirectChild rules (noise from recursive nesting)

These rules say "X should be in Y, not X" — the component appears as both
the child and the wrong parent because it supports recursive nesting
(e.g., MenuItem can contain a nested MenuList->MenuItem structure).

| Family | Rule | Status |
|--------|------|--------|
| Menu | MenuItem should be in MenuList, not MenuItem | Correct noise — MenuItem supports nested sub-menus |
| Menu | MenuContent should be in Menu, not MenuContent | Correct noise — recursive Menu nesting |
| Menu | DrilldownMenu should be in Menu, not DrilldownMenu | Correct noise — drilldown recursion |
| Dropdown | DropdownItem should be in DropdownList, not DropdownItem | Correct noise — delegated from Menu recursive pattern |
| Select | SelectOption should be in SelectList, not SelectOption | Correct noise — delegated from Menu recursive pattern |
| Wizard | WizardNavItem should be in WizardNav, not WizardNavItem | Correct noise — sub-navigation recursion |
| deprecated/Wizard | WizardNavItem should be in WizardNav, not WizardNavItem | Same |

### Wrong requiresChild rules

| Family | Rule | Status | Fix |
|--------|------|--------|-----|
| ChartBullet | ChartBullet must contain ChartBulletComparativeErrorMeasure (+ 6 more) | Not emitted — prop_passed edges are excluded from PMC maps in conformance rule generation | Edge strength is still Wrapper (wrong) but rule gen filters it correctly |
| DescriptionList | DescriptionList must contain DescriptionListTerm | FIXED — Step 9.6 suppresses DOM shortcut edges | — |
| DescriptionList | DescriptionList must contain DescriptionListTermHelpText | FIXED — same | — |

### Wrong-level edges (parent-child at wrong depth)

| Family | Edge | Strength | Status | Reason |
|--------|------|----------|--------|--------|
| DescriptionList | DescriptionList->DescriptionListTerm | — | FIXED — suppressed by Step 9.6 Path 2 | Required wrapper (DLGroup) provides path to child |
| DescriptionList | DescriptionList->DescriptionListTermHelpText | — | FIXED — same | — |
| Table | Tbody->Td | allowed | Noise — Allowed, no conformance rule generated | CSS descendant `.tbody .td` matches at any depth |
| Table | Thead->Td | allowed | Same | |
| Table | Thead->SortColumn | allowed | Same | |
| Table | Table->TableText | allowed | Same | |
| Table | Table->CollapseColumn | allowed | Same | |
| Table | Table->HeaderCellInfoWrapper | allowed | Same | |
| Alert | AlertGroup->AlertActionCloseButton | — | FIXED — Step 6 context edge skipped for prop-passed children | AlertActionCloseButton is prop-passed to Alert via `actionClose`; context dependency on AlertGroupContext is ambient |
| Dropdown | DropdownItem->DropdownList | allowed | Noise — recursive nesting for sub-menus (delegate from Menu) | |
| Dropdown | DropdownItem->Dropdown | allowed | Same | |
| Select | SelectOption->SelectList | allowed | Same | |
| DataList | DataListContent->DataList | allowed | Noise — CSS `>` from expandable content back to root | |
| Menu | MenuContent->Menu | allowed | Noise — CSS descendant recursive match | |

### Edges TO the family root component

Some edges point TO the family root from a child. Most are valid
composition relationships (e.g., AlertGroup contains Alerts where
Alert is the family root).

| Family | Edge | Strength | Status |
|--------|------|----------|--------|
| Alert | AlertGroup->Alert | structural | Correct — AlertGroup contains Alerts |
| ChartDonutUtilization | ChartDonutThreshold->ChartDonutUtilization | wrapper | FIXED — Step 8 uses Wrapper for ReactElement children type (Cat C: PMC=YES, CHP=NO) |
| ExpandableSection | ExpandableSectionToggle->ExpandableSection | allowed | Noise — CSS match, no rule generated |
| JumpLinks | JumpLinksList->JumpLinks | allowed | Noise — CSS `.list .list` recursive match |
| Menu | MenuItem->Menu | allowed | Noise — recursive sub-menu nesting |
| Menu | MenuBreadcrumb->Menu | allowed | Noise — CSS descendant match |
| DataList | DataListContent->DataList | allowed | Noise — CSS `>` expandable content |
| Dropdown | DropdownItem->Dropdown | allowed | Noise — delegate projection recursive |
| Select | SelectOption->Select | allowed | Noise — delegate projection recursive |

### Wrong requiresChild regex (noise children in regex)

The `requiresChild` rule message lists ALL children in the regex as
"must contain", which is misleading even though the scanner regex needs
them for OR semantics (to prevent false negatives when valid children
are present). These are not false positives but misleading guidance.

| Family | Rule | Noise Children | Valid Children |
|--------|------|---------------|----------------|
| Table | Table must contain ... | CollapseColumn, HeaderCellInfoWrapper, TableText | Caption, Tbody, Thead |
| Table | Thead must contain ... | SortColumn, Td | Tr |
| DataList | DataList must contain ... | Action, Check, Content, Control, DragButton, Text | DataListItem |

### Missing requiresChild children (false positive risk)

| Family | Rule | Missing Child | Impact |
|--------|------|--------------|--------|
| FormSelect | FormSelect must contain FormSelectOptionGroup | FormSelectOption | Direct `<FormSelectOption>` children (without optgroup) falsely trigger the rule |

### Tree anomalies (from composition tree scan)

| Family | Issue | Severity |
|--------|-------|----------|
| deprecated/Table | Self-loop: `Header -> Header` (wrapper, internal) | Bug — parser confusion on component name collision |
| Wizard | Bidirectional cycle: `WizardNav <-> WizardNavItem` | FIXED — Step 8.8 downgrades weaker direction (Structural) to Allowed |
| deprecated/Wizard | Same cycle as Wizard | FIXED — same |
| Form | Orphan exports: FormFieldGroup, FormFieldGroupExpandable have zero edges | Missing edges — consumer-placed components with no composition signal |

### Wizard/deprecated duplicate `when` clauses

FIXED — Deprecated conformance rules now use the deprecated import path
in their `from` field (e.g., `@patternfly/react-core/deprecated` instead of
`@patternfly/react-core`). The `pkg_for_deprecated` helper appends
`/deprecated` to the base package for families whose root starts with
`"deprecated/"`, unless the package already contains `/deprecated` (to
prevent double-appending for families like deprecated/Table where the
component_packages map already resolves correctly).

This scopes deprecated rules to deprecated imports, preventing duplicate
violations with v6 rules that share the same component names.
