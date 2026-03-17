# Integration Pipeline: Auto-Fix vs Hand-Migrated Comparison

## Pipeline Summary

| Step | Output |
|---|---|
| PatternFly analysis | `/tmp/semver-integration/patternfly-report.json` |
| Konveyor rules (builtin) | `/tmp/semver-integration/konveyor-rules-builtin/` |
| Konveyor rules (frontend) | `/tmp/semver-integration/konveyor-rules-frontend/` |
| Fix guidance | `/tmp/semver-integration/fix-guidance/` |
| Kantra output (hand-crafted) | `/tmp/semver-integration/kantra-output/` |
| Kantra output (auto-generated) | `/tmp/semver-integration/kantra-output-auto/` |
| Auto-fixed quipucords | `/tmp/semver-integration/quipucords-ui-fixed/` |
| Hand-migrated quipucords | `/tmp/semver-integration/quipucords-ui-v6/` |
| Full diff | `/tmp/semver-integration/auto-vs-hand-migrated.diff` |

## Breaking Changes Detected

| Metric | Count |
|---|---|
| Total breaking changes | 10509 |
| API changes | 10178 |
| Behavioral changes | 331 |
| Files with changes | 6307 |

## Kantra Analysis: Hand-Crafted vs Auto-Generated Rules

| Metric | Hand-Crafted Rules | Auto-Generated Rules |
|---|---|---|
| Violation types | 26 | 58 |
| Total incidents | 176 | 683 |

### Hand-Crafted Rule Violations

| Rule ID | Description | Incidents |
|---|---|---|
| - | Button: icon children should use icon prop instead | 5 |
| - | Title: default size mapping changed | 4 |
| - | PF v5 CSS class prefix pf-v5-c- still in use | 7 |
| - | PF v6 CSS variables use logical properties (BlockStart/InlineEnd) | 1 |
| - | PF v5 CSS variable prefix --pf-v5- still in use | 2 |
| - | Old Modal API: uses title/actions props instead of ModalHeader/ModalBody | 19 |
| - | Old Select components must be migrated to new composable Select | 7 |
| - | DropdownItem: tooltip rendered as sibling | 4 |
| - | Masthead: MastheadToggle is direct child of Masthead (should be inside MastheadMain) | 1 |
| - | NavList: scroll buttons wrapped and conditionally rendered | 1 |
| - | Pagination: wrapper added around options menu toggle | 7 |
| - | PageToggleButton: BarsIcon child should use isHamburgerButton prop | 1 |
| - | PF v6 requires separate utilities CSS import | 1 |
| - | Nav: theme prop removed | 2 |
| - | innerRef replaced with standard React ref | 6 |
| - | Page: header renamed to masthead | 1 |
| - | Toolbar: align values alignLeft/alignRight renamed | 1 |
| - | Toolbar: chip-based props renamed to label-based | 6 |
| - | PageSection: variant values narrowed | 3 |
| - | ToolbarGroup: icon-button-group variant renamed | 2 |
| - | Toolbar: spacer/spaceItems replaced with gap props | 2 |
| - | EmptyStateHeader and EmptyStateIcon merged into EmptyState | 13 |
| - | MastheadBrand: children should be wrapped in MastheadLogo | 1 |
| - | Text/TextContent/TextList/TextListItem replaced with Content | 1 |
| - | Snapshot files contain stale PF v5 class names | 73 |
| - | react-tokens: global_* token imports need updating | 5 |

### Auto-Generated Rule Violations

| Rule ID | Description | Incidents |
|---|---|---|
| - | CSS class 'styles.modifiers.plain)' added to render output | 35 |
| - | Return type of `variant` changed from `'control' | 'danger' | 'link' | 'plain' | 'primary' | 'secondary' | 'tertiary' | 'warning'` to `'control' | 'danger' | 'link' | 'plain' | 'primary' | 'secondary' | 'stateful' | 'tertiary' | 'warning'` | 16 |
| - | Return type of `icon` changed from `ReactElement<EmptyStateIconProps>` to `ComponentType<any>` | 1 |
| - | property `actions` was removed from `ModalProps` | 7 |
| - | property `title` was removed from `ModalProps` | 12 |
| - | property `theme` was removed from `NavProps` | 1 |
| - | property `header` was renamed to `masthead` in `PageProps` | 1 |
| - | Return type of `variant` changed from `'dark' | 'darker' | 'default' | 'light'` to `'default' | 'secondary'` | 3 |
| - | property `theme` was removed from `PageSidebarProps` | 1 |
| - | property `variant` was added to `SelectProps` | 1 |
| - | Exported constant `TextContent` was removed | 1 |
| - | Exported constant `TextList` was removed | 1 |
| - | Exported constant `TextListItem` was removed | 1 |
| - | Return type of `categoryName` changed from `ToolbarChipGroup | string` to `ToolbarLabelGroup | string` | 3 |
| - | property `chips` was removed from `ToolbarFilterProps` | 3 |
| - | property `deleteChip` was removed from `ToolbarFilterProps` | 3 |
| - | Return type of `align` changed from `{ default?: 'alignLeft' | 'alignRight'; md?: 'alignLeft' | 'alignRight'; lg?: 'alignLeft' | 'alignRight'; xl?: 'alignLeft' | 'alignRight'; ['2xl']?: 'alignLeft' | 'alignRight' }` to `{ default?: 'alignCenter' | 'alignEnd' | 'alignStart'; md?: 'alignCenter' | 'alignEnd' | 'alignStart'; lg?: 'alignCenter' | 'alignEnd' | 'alignStart'; xl?: 'alignCenter' | 'alignEnd' | 'alignStart'; ['2xl']?: 'alignCenter' | 'alignEnd' | 'alignStart' }` | 1 |
| - | property `spacer` was removed from `ToolbarGroupProps` | 1 |
| - | Return type of `variant` changed from `'button-group' | 'filter-group' | 'icon-button-group' | ToolbarGroupVariant` to `'action-group' | 'action-group-inline' | 'action-group-plain' | 'filter-group' | 'label-group' | ToolbarGroupVariant` | 2 |
| - | property `spaceItems` was removed from `ToolbarToggleGroupProps` | 1 |
| - | Exported constant `Dropdown` was removed | 5 |
| - | Exported constant `DropdownGroup` was removed | 1 |
| - | Exported constant `DropdownItem` was removed | 5 |
| - | property `actions` was added to `ModalProps` | 7 |
| - | property `title` was added to `ModalProps` | 12 |
| - | Exported variable `Select` was removed | 3 |
| - | Exported variable `SelectOption` was removed | 3 |
| - | variable `Modal` moved to deprecated exports | 9 |
| - | CSS class '`${styles.expandableSection}__toggle`' added to render output | 35 |
| - | <EyeSlashIcon> element removed from render output (1 instance) | 1 |
| - | CSS class 'styles.modifiers.splitButton)' added to render output | 35 |
| - | CSS class 'styles.modifiers.subnav,' added to render output | 35 |
| - | 1 additional <span> wrapper element added | 1 |
| - | CSS class 'styles.modifiers.fill)' added to render output | 35 |
| - | CSS class 'styles.modifiers.on)' removed from render output | 35 |
| - | CSS class 'styles.modifiers.subtab,' added to render output | 35 |
| - | CSS class 'styles.modifiers[HeadingLevel],' added to render output | 2 |
| - | aria-label attribute added to <button> | 2 |
| - | CSS class 'styles.modifiers.alignItemsBaseline' added to render output | 35 |
| - | CSS class 'styles.modifiers.chipContainer' removed from render output | 35 |
| - | CSS class 'styles.modifiers[' added to render output | 35 |
| - | CSS class 'styles.modifiers[' added to render output | 35 |
| - | CSS class 'className' added to render output | 35 |
| - | CSS class 'className' added to render output | 35 |
| - | CSS class 'cardWithActions' added to render output | 35 |
| - | CSS class 'notification-1' added to render output | 35 |
| - | CSS class 'notification-9' added to render output | 35 |
| - | Exported constant `global_Color_dark_100` was removed | 1 |
| - | Exported constant `global_danger_color_100` was removed | 1 |
| - | Exported constant `global_danger_color_200` was removed | 1 |
| - | Exported constant `global_success_color_100` was removed | 1 |
| - | Exported constant `global_warning_color_100` was removed | 1 |
| - | Exported variable `global_Color_dark_100` was removed | 1 |
| - | Exported variable `global_danger_color_100` was removed | 1 |
| - | Exported variable `global_danger_color_200` was removed | 1 |
| - | Exported variable `global_success_color_100` was removed | 1 |
| - | Exported variable `global_warning_color_100` was removed | 1 |
| - | CSS class prefix 'pf-v5-theme-dark' removed from source | 2 |

## Auto-Fixed vs Hand-Migrated Diff

| Metric | Value |
|---|---|
| Files changed by auto-fix | 0 |
| Files changed in hand migration (v5→v6) | 132 |
| Diff lines (auto-fixed vs hand-migrated) | 13468 |

### Per-File Differences (auto-fixed vs hand-migrated)

```
Files /tmp/semver-integration/quipucords-ui-fixed/src/app.css and /tmp/semver-integration/quipucords-ui-v6/src/app.css differ
Files /tmp/semver-integration/quipucords-ui-fixed/src/app.tsx and /tmp/semver-integration/quipucords-ui-v6/src/app.tsx differ
Files /tmp/semver-integration/quipucords-ui-fixed/src/components/aboutModal/aboutModal.tsx and /tmp/semver-integration/quipucords-ui-v6/src/components/aboutModal/aboutModal.tsx differ
Files /tmp/semver-integration/quipucords-ui-fixed/src/components/actionMenu/actionMenu.tsx and /tmp/semver-integration/quipucords-ui-v6/src/components/actionMenu/actionMenu.tsx differ
Files /tmp/semver-integration/quipucords-ui-fixed/src/components/contextIcon/contextIcon.tsx and /tmp/semver-integration/quipucords-ui-v6/src/components/contextIcon/contextIcon.tsx differ
Files /tmp/semver-integration/quipucords-ui-fixed/src/components/errorMessage/errorMessage.tsx and /tmp/semver-integration/quipucords-ui-v6/src/components/errorMessage/errorMessage.tsx differ
Files /tmp/semver-integration/quipucords-ui-fixed/src/components/i18n/__test__/i18n.test.tsx and /tmp/semver-integration/quipucords-ui-v6/src/components/i18n/__test__/i18n.test.tsx differ
Files /tmp/semver-integration/quipucords-ui-fixed/src/components/i18n/i18n.tsx and /tmp/semver-integration/quipucords-ui-v6/src/components/i18n/i18n.tsx differ
Files /tmp/semver-integration/quipucords-ui-fixed/src/components/login/login.tsx and /tmp/semver-integration/quipucords-ui-v6/src/components/login/login.tsx differ
Only in /tmp/semver-integration/quipucords-ui-v6/src/components: secretInput
Files /tmp/semver-integration/quipucords-ui-fixed/src/components/simpleDropdown/simpleDropdown.tsx and /tmp/semver-integration/quipucords-ui-v6/src/components/simpleDropdown/simpleDropdown.tsx differ
Files /tmp/semver-integration/quipucords-ui-fixed/src/components/typeAheadCheckboxes/__tests__/typeaheadCheckboxes.test.tsx and /tmp/semver-integration/quipucords-ui-v6/src/components/typeAheadCheckboxes/__tests__/typeaheadCheckboxes.test.tsx differ
Files /tmp/semver-integration/quipucords-ui-fixed/src/components/typeAheadCheckboxes/typeaheadCheckboxes.tsx and /tmp/semver-integration/quipucords-ui-v6/src/components/typeAheadCheckboxes/typeaheadCheckboxes.tsx differ
Files /tmp/semver-integration/quipucords-ui-fixed/src/components/viewLayout/__tests__/viewLayoutToolbar.test.tsx and /tmp/semver-integration/quipucords-ui-v6/src/components/viewLayout/__tests__/viewLayoutToolbar.test.tsx differ
Files /tmp/semver-integration/quipucords-ui-fixed/src/components/viewLayout/__tests__/viewLayoutToolbarInteractions.test.tsx and /tmp/semver-integration/quipucords-ui-v6/src/components/viewLayout/__tests__/viewLayoutToolbarInteractions.test.tsx differ
Files /tmp/semver-integration/quipucords-ui-fixed/src/components/viewLayout/viewLayout.tsx and /tmp/semver-integration/quipucords-ui-v6/src/components/viewLayout/viewLayout.tsx differ
Files /tmp/semver-integration/quipucords-ui-fixed/src/components/viewLayout/viewLayoutToolbar.css and /tmp/semver-integration/quipucords-ui-v6/src/components/viewLayout/viewLayoutToolbar.css differ
Files /tmp/semver-integration/quipucords-ui-fixed/src/components/viewLayout/viewLayoutToolbar.tsx and /tmp/semver-integration/quipucords-ui-v6/src/components/viewLayout/viewLayoutToolbar.tsx differ
Files /tmp/semver-integration/quipucords-ui-fixed/src/constants/apiConstants.ts and /tmp/semver-integration/quipucords-ui-v6/src/constants/apiConstants.ts differ
Files /tmp/semver-integration/quipucords-ui-fixed/src/helpers/__tests__/apiHelpers.test.ts and /tmp/semver-integration/quipucords-ui-v6/src/helpers/__tests__/apiHelpers.test.ts differ
Files /tmp/semver-integration/quipucords-ui-fixed/src/helpers/__tests__/helpers.test.ts and /tmp/semver-integration/quipucords-ui-v6/src/helpers/__tests__/helpers.test.ts differ
Files /tmp/semver-integration/quipucords-ui-fixed/src/helpers/apiHelpers.ts and /tmp/semver-integration/quipucords-ui-v6/src/helpers/apiHelpers.ts differ
Files /tmp/semver-integration/quipucords-ui-fixed/src/helpers/helpers.ts and /tmp/semver-integration/quipucords-ui-v6/src/helpers/helpers.ts differ
Files /tmp/semver-integration/quipucords-ui-fixed/src/hooks/__tests__/useCredentialApi.test.ts and /tmp/semver-integration/quipucords-ui-v6/src/hooks/__tests__/useCredentialApi.test.ts differ
Files /tmp/semver-integration/quipucords-ui-fixed/src/hooks/__tests__/useLoginApi.test.ts and /tmp/semver-integration/quipucords-ui-v6/src/hooks/__tests__/useLoginApi.test.ts differ
Files /tmp/semver-integration/quipucords-ui-fixed/src/hooks/__tests__/useScanApi.test.ts and /tmp/semver-integration/quipucords-ui-v6/src/hooks/__tests__/useScanApi.test.ts differ
Files /tmp/semver-integration/quipucords-ui-fixed/src/hooks/__tests__/useSourceApi.test.ts and /tmp/semver-integration/quipucords-ui-v6/src/hooks/__tests__/useSourceApi.test.ts differ
Files /tmp/semver-integration/quipucords-ui-fixed/src/hooks/__tests__/useStatusApi.test.ts and /tmp/semver-integration/quipucords-ui-v6/src/hooks/__tests__/useStatusApi.test.ts differ
Files /tmp/semver-integration/quipucords-ui-fixed/src/hooks/useCredentialApi.ts and /tmp/semver-integration/quipucords-ui-v6/src/hooks/useCredentialApi.ts differ
Files /tmp/semver-integration/quipucords-ui-fixed/src/hooks/useLoginApi.ts and /tmp/semver-integration/quipucords-ui-v6/src/hooks/useLoginApi.ts differ
Files /tmp/semver-integration/quipucords-ui-fixed/src/hooks/useScanApi.ts and /tmp/semver-integration/quipucords-ui-v6/src/hooks/useScanApi.ts differ
Files /tmp/semver-integration/quipucords-ui-fixed/src/hooks/useSourceApi.ts and /tmp/semver-integration/quipucords-ui-v6/src/hooks/useSourceApi.ts differ
Only in /tmp/semver-integration/quipucords-ui-v6/src/images: overviewSecurity-dark.svg
Only in /tmp/semver-integration/quipucords-ui-v6/src/images: overviewSecurity.svg
Only in /tmp/semver-integration/quipucords-ui-v6/src/images: titleBrandLogin.svg
Only in /tmp/semver-integration/quipucords-ui-v6/src/images: titleLogin.svg
Only in /tmp/semver-integration/quipucords-ui-v6/src: locales
Files /tmp/semver-integration/quipucords-ui-fixed/src/routes.tsx and /tmp/semver-integration/quipucords-ui-v6/src/routes.tsx differ
Files /tmp/semver-integration/quipucords-ui-fixed/src/types/types.ts and /tmp/semver-integration/quipucords-ui-v6/src/types/types.ts differ
Only in /tmp/semver-integration/quipucords-ui-v6/src/vendor: VENDORED_CHANGES.md
```

### Fix Guidance Summary

```yaml
total_fixes: 10510
auto_fixable: 2767
needs_review: 51
manual_only: 7692
```

## Conclusions

### What the auto-generated pipeline covers
- API-level breaking changes (renamed/removed components, props, types)
- DOM structure changes (wrapper elements, element type changes)
- Accessibility changes (ARIA attributes, role changes)
- CSS class and variable renames
- Fix guidance with strategy, confidence, and concrete instructions

### What still requires manual attention
- Prop value changes (e.g., `variant="tertiary"` → `variant="horizontal-subnav"`)
- Complex structural refactors (e.g., Modal → ModalHeader/ModalBody composition)
- CSS-in-JS style changes
- Changes to non-PatternFly dependencies made during the migration
- New feature additions in the hand-migrated version

### Key metric
The auto-fix pipeline can reduce the manual migration effort by addressing
the mechanical changes (renames, removals, import path fixes) automatically,
leaving only the structural refactors and value migrations for human review.
