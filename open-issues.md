# Open Issues

## Bugs

### 1. DualListSelector composition guidance generates incorrect migration instructions

**Severity:** High

The composition rule `semver-new-sibling-duallistselectorcontrol-in-duallistselector` generates incorrect migration guidance. The rule message says:

```
addSelected → <DualListSelectorControl addSelected={...}>
```

This is wrong. In PF v5, `addSelected` was a **callback prop on `DualListSelector`** (the parent component) — it was never a prop on `DualListSelectorControl`. In PF v6, `addSelected` was **removed entirely**, not moved to a child component.

**Verified against upstream source:**

- PF v5 `DualListSelector.tsx`: `addSelected?: (newAvailableOptions: React.ReactNode[], newChosenOptions: React.ReactNode[]) => void` — a callback on the parent
- PF v5 `DualListSelectorControl.tsx`: No `addSelected` prop — only `icon`, `onClick`, `isDisabled`, `tooltipContent`, etc.
- PF v6 `DualListSelectorControl.tsx`: Same props as v5, still no `addSelected`

**Impact:** The LLM follows this incorrect guidance and produces code like `<DualListSelectorControl addSelected />` which fails to compile. The correct migration is to use `<DualListSelectorControl onClick={yourHandler}>` and move the selection logic into the `onClick` callback.

**Root cause:** The composition rule generator incorrectly classifies removed parent props as "moved to child component" when the child component was introduced alongside the parent refactoring. The generator should distinguish between:

- Props that were **moved** to a child (the child's interface actually has the prop)
- Props that were **removed** and replaced by a different pattern (e.g., composition with `onClick`)

**Affected props:** `addSelected`, `addSelectedAriaLabel`, `addSelectedTooltip`, `addSelectedTooltipProps`, `addAll`, `addAllAriaLabel`, `addAllTooltip`, `addAllTooltipProps`, `removeSelected`, `removeSelectedAriaLabel`, `removeSelectedTooltip`, `removeSelectedTooltipProps`, `removeAll`, `removeAllAriaLabel`, `removeAllTooltip`, `removeAllTooltipProps` — all listed as "moved to DualListSelectorControl" but none actually exist on `DualListSelectorControlProps`.

---

### 2. Pagination compact rule message is misleading, causes incorrect LLM fixes

**Severity:** Medium

Rule `semver-packages-react-core-src-components-pagination-pagination-tsx-pagination-behavioral` describes the behavioral change as:

> The isCompact prop no longer applies the 'pf-m-compact' CSS modifier class. The line `isCompact && styles.modifiers.compact` was removed from the className computation.

This is technically accurate but misleading. It doesn't mention that the `pf-m-compact` CSS class was **removed from the PF v6 stylesheet entirely**. The CSS module `@patternfly/react-styles/css/components/Pagination/pagination` no longer exports `modifiers.compact`.

**Impact:** The LLM interprets this as "the prop doesn't work anymore, but the CSS class still exists — apply it manually." This produces code like:

```tsx
className={`${isCompact ? ` ${styles.modifiers.compact}` : ""}`}
```

which fails to compile because `styles.modifiers.compact` doesn't exist in PF v6.

**Confirmed via goose log:** `goose-fix-043.json` shows goose processed `SimplePagination.tsx`, removed `isCompact={isCompact}`, and added a manual `styles.modifiers.compact` application — producing the exact TypeScript error.

**Suggested fix:** Update the rule message to explicitly state:

> The `pf-m-compact` CSS modifier class has been removed from the Pagination stylesheet. The `isCompact` prop and `styles.modifiers.compact` are no longer available. Remove all references to compact pagination styling.
