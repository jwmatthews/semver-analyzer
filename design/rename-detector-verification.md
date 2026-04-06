# Rename Detector Verification Guide

This document is the authoritative reference for validating changes to the
rename detection algorithm in `crates/core/src/diff/rename.rs`. It contains:

1. How the rename detector works
2. PatternFly v5 → v6 change landscape (context)
3. Verified true renames (must be preserved)
4. Verified false renames (must be eliminated)
5. Threshold analysis and score distributions
6. Verification procedure
7. Known gaps and future work

---

## 1. How the Rename Detector Works

### Location

`crates/core/src/diff/rename.rs` — function `detect_renames()`

### Algorithm

The detector takes two lists (removed symbols, added symbols) and a
`same_family` closure, then returns candidate rename matches. It runs
**4 passes**, each progressively more relaxed:

| Pass | Fingerprint Method | Min Similarity | Scope |
|------|-------------------|----------------|-------|
| 1 | Exact `MemberFingerprint` (kind + exact return_type + is_optional + param_count) | 0.15 (same-family) / 0.50 (cross-family) | Global |
| 2 | Normalized fingerprint (PascalCase types → `_T_`, param names → `_p_`) | 0.15 / 0.50 | Global |
| 3 | Deep normalized (additionally string literals → `_V_`) | 0.15 / 0.50 | Global |
| 4 | Name similarity only | 0.60 | Same parent interface |

### Fingerprint Structure

```rust
struct MemberFingerprint {
    kind: SymbolKind,              // Constant, Interface, TypeAlias, Property, etc.
    return_type: Option<String>,   // Normalized type string
    is_optional: bool,
    param_count: usize,
}
```

### Name Similarity

Computed as LCS (Longest Common Subsequence) ratio:

```
similarity = LCS_length(old_name, new_name) / max(len(old_name), len(new_name))
```

### Cross-Family Guard

In Passes 1–3, after computing similarity for a candidate pair, the detector
checks `same_family(old, new)`:

- **Same family** → minimum similarity threshold is `MIN_SIMILARITY` (0.15)
- **Different family** → minimum similarity threshold is
  `CROSS_FAMILY_MIN_SIMILARITY` (0.50)

The `same_family()` function (implemented in `crates/ts/src/language.rs`)
compares canonical component directories by stripping `/deprecated/` and
`/next/` path segments, then comparing the parent directory.

### Matching

After all passes, candidates are sorted by similarity (descending) and greedily
assigned. Each symbol is used at most once (1:1 matching).

### Post-Filtering

After `detect_renames` returns, `mod.rs` applies `types_structurally_similar()`
to reject type-incompatible renames. However, this check is coarse — both
`FunctionComponent<AProps>` and `FunctionComponent<BProps>` are
`TypeCategory::Reference` and pass.

---

## 2. PatternFly v5 → v6 Change Landscape

### Overall Distribution (v5.4.0 → v6.4.1)

| Change Type | Count | % |
|---|---|---|
| removed | 7,365 | 47.4% |
| renamed | 4,094 | 26.4% |
| type_changed | 3,866 | 24.9% |
| signature_changed | 200 | 1.3% |
| **Total** | **15,525** | |

### Removals (340 non-token)

| Category | Count | Examples |
|---|---|---|
| Removed components | 16 | Text, TextContent, EmptyStateHeader, EmptyStateIcon, PageNavigation |
| Removed props | 199 | DualListSelector (44), Toolbar* (33), Popper (32), Modal (11) |
| Removed types/interfaces | 46 | 34 chart Props (path change), 12 react-core |
| Removed from deprecated/ | 79 | 42 constants + 35 interfaces + 2 type aliases |

### Renames (4,094 total)

| Category | Count | Correct | False |
|---|---|---|---|
| Prop renames | 17 | 13 | 4 |
| Type/interface renames | 8 | 1 | 7 |
| Component path relocations | 34 | 34 | 0 |
| Import path changes (charts) | 41 | 41 | 0 |
| CSS token renames | 3,995 | 3,995 | 0 |

### Type Changes (3,866)

Key patterns:

- RefObject<T> → RefObject<T | null> (React 19 compat): 14 props
- ReactElement → ReactElement\<any\> (strict mode): 12 props
- onSelect value narrowing (number | string → Props['value']): 9 components
- Color variant renames ('light-200' → 'secondary'): 5 props
- Enum value additions/removals: ~20 props
- CSS token type changes: 3,774

### Signature Changes (200)

- 184 new props added across 60 components
- 13 interface base class changes
- 3 constants made readonly

---

## 3. Verified TRUE Renames (15)

All of these MUST continue to be detected after any algorithm change.

### Same-Family Prop Renames (14)

| Old | New | Similarity | Component | Family Dir |
|---|---|---|---|---|
| `usePageInsets` | `hasNoPadding` | 0.308 | Toolbar | Toolbar/ |
| `tertiaryNav` | `horizontalSubnav` | 0.313 | Page | Page/ |
| `isSecondary` | `isSubtab` | 0.364 | Tabs | Tabs/ |
| `isActive` | `isClicked` | 0.444 | Button | Button/ |
| `header` | `masthead` | 0.500 | Page | Page/ |
| `isTertiaryNavGrouped` | `isHorizontalSubnavGrouped` | 0.560 | Page | Page/ |
| `selectOptions` | `initialOptions` | 0.571 | TypeaheadSelect | Select/ |
| `isTertiaryNavWidthLimited` | `isHorizontalSubnavWidthLimited` | 0.633 | Page | Page/ |
| `chipContainerRef` | `labelContainerRef` | 0.706 | ToolbarExpandableContent | Toolbar/ |
| `chipContainerRef` | `labelContainerRef` | 0.706 | ToolbarToggleGroup | Toolbar/ |
| `chipGroupExpandedText` | `labelGroupExpandedText` | 0.773 | ToolbarFilter | Toolbar/ |
| `customChipGroupContent` | `customLabelGroupContent` | 0.783 | Toolbar | Toolbar/ |
| `customChipGroupContent` | `customLabelGroupContent` | 0.783 | ToolbarContext | Toolbar/ |
| `chipGroupCollapsedText` | `labelGroupCollapsedText` | 0.783 | ToolbarFilter | Toolbar/ |

### Cross-Family Type Renames (1)

| Old | New | Similarity | Old Dir | New Dir |
|---|---|---|---|---|
| `TextVariants` | `ContentVariants` | 0.667 | Text/ | Content/ |

---

## 4. Verified FALSE Renames (28)

All of these SHOULD NOT be detected. They are symbols removed in v6 that were
incorrectly matched to unrelated new symbols.

### Cross-Family Type Alias Matches (7)

| Old | New | Similarity | Old Dir | New Dir | Root Cause |
|---|---|---|---|---|---|
| `TextListVariants` | `ToolbarColorVariant` | 0.421 | Text/ | Toolbar/ | Both type_alias with no return_type → identical normalized fingerprint |
| `TextListItemVariants` | `HelperTextItemVariant` | 0.714 | Text/ | HelperText/ | Same fingerprint; high similarity from shared "Text" and "Item" |
| `DropdownDirection` | `DrawerContentColorVariant` | 0.320 | Dropdown/ | Drawer/ | Same fingerprint |
| `SelectDirection` | `MenuToggleSize` | 0.333 | Select/ | MenuToggle/ | Same fingerprint |
| `SelectPosition` | `ButtonState` | 0.286 | Select/ | Button/ | Same fingerprint |
| `SelectVariant` | `SidebarBackgroundVariant` | 0.417 | Select/ | Sidebar/ | Same fingerprint; shared "Variant" suffix |
| `OptionsMenuPosition` | `EmptyStateStatus` | 0.263 | OptionsMenu/ | EmptyState/ | Same fingerprint |

**Note**: `TextListItemVariants` → `HelperTextItemVariant` has similarity 0.714,
which is above the 0.50 cross-family threshold. This specific false rename
requires additional guards beyond the threshold (see Section 7).

### Cross-Family Component/Constant Matches (4)

| Old | New | Similarity | Old Dir | New Dir | Root Cause |
|---|---|---|---|---|---|
| `DropdownSeparator` | `DrawerPanelDescription` | 0.364 | Dropdown/ | Drawer/ | Both `FunctionComponent<_T_>` after normalization |
| `SeparatorProps` | `ChartsProps` | 0.222 | Dropdown/ | Charts/ | Both interface with no return_type |
| `DropdownToggleActionProps` | `DrawerPanelDescriptionProps` | 0.400 | Dropdown/ | Drawer/ | Both interface |
| `SplitButtonOptions` | `PopperOptions` | 0.308 | MenuToggle/ | Popper/ | Both interface; shared "Options" suffix |

### Same-Family False Prop Matches (3)

| Old | New | Similarity | Component | Root Cause |
|---|---|---|---|---|
| `isDisabled` | `hasAnimations` | 0.231 | DualListSelector | Both boolean props → identical fingerprint; greedy matcher picks best available |
| `isOverflowLabel` | `isClickable` | 0.400 | Label | Both boolean props |
| `bodyAriaLabel` | `backdropClassName` | 0.353 | Modal | Both string props |

**These cannot be fixed by a cross-family threshold** — they are same-component.
They require fingerprint collision handling within same-interface (Phase 2 work).

### Additional False Renames (v1 report only, 14)

These appear only in the v1 (BU) pipeline report:

| Old | New | Old Dir | New Dir |
|---|---|---|---|
| EmptyStateIcon | PenToSquareIcon | EmptyState/ | Icons/ |
| PageHeader | PageBody | PageHeader/ | Page/ |
| PageHeaderTools | MastheadLogo | PageHeader/ | Masthead/ |
| PageHeaderProps | PageBodyProps | PageHeader/ | Page/ |
| PageHeaderToolsProps | MastheadLogoProps | PageHeader/ | Masthead/ |
| ApplicationLauncherProps | AnimationsProviderProps | ApplicationLauncher/ | Animations/ |
| ApplicationLauncherText | FileUploadHelperText | ApplicationLauncher/ | FileUpload/ |
| ApplicationLauncherTextProps | FileUploadHelperTextProps | ApplicationLauncher/ | FileUpload/ |
| PageNavigationProps | AnimationsConfig | Page/ | Animations/ |
| TextProps | TruncateProps | Text/ | Truncate/ |
| TextList | Charts | Text/ | Charts/ |
| OptionsMenuProps | ChartsOptionProps | OptionsMenu/ | Charts/ |
| OptionsMenuItemProps | TooltipOptionProps | OptionsMenu/ | Tooltip/ |
| OptionsMenuItemGroupProps | FormGroupLabelHelpProps | OptionsMenu/ | FormGroup/ |

---

## 5. Threshold Analysis

### Score Distributions

```
TRUE renames (same-family):  0.308 ─────────────────────────────── 0.783
TRUE renames (cross-family): ................. 0.667
FALSE renames (cross-family):0.222 ────────────────── 0.714
FALSE renames (same-family): 0.231 ─────── 0.400
                             0.0   0.2   0.4   0.6   0.8   1.0
```

### Key Boundaries

| Boundary | Value | Notes |
|---|---|---|
| Lowest true same-family | 0.308 | `usePageInsets` → `hasNoPadding` |
| Highest false same-family | 0.400 | `isOverflowLabel` → `isClickable` |
| **Gap (same-family)** | **NEGATIVE** | True and false distributions overlap |
| Lowest true cross-family | 0.667 | `TextVariants` → `ContentVariants` |
| Highest false cross-family | 0.714 | `TextListItemVariants` → `HelperTextItemVariant` |
| **Gap (cross-family)** | **NEGATIVE** | True and false distributions overlap |

### Implication

A simple similarity threshold ALONE cannot separate true from false renames in
either the same-family or cross-family case. The threshold must be combined
with other signals:

- **Cross-family**: A threshold of 0.50 catches 10 of 11 cross-family false
  renames (all except `TextListItemVariants` at 0.714). For the remaining one,
  additional heuristics are needed (e.g., deprecated-to-non-deprecated family
  mismatch, or type-alias-specific guards).
- **Same-family**: Cannot use similarity alone. Need fingerprint disambiguation
  (e.g., when many boolean props are removed and added, use structural context
  beyond the type).

---

## 6. Verification Procedure

When modifying the rename detector, follow this procedure:

### Step 1: Run PF v5→v6 Analysis

```sh
# Build
cargo build

# Run analysis (requires patternfly-react repo)
./target/debug/semver-analyzer analyze typescript \
    --repo /path/to/patternfly-react \
    --from v5.4.0 --to v6.4.1 \
    --no-llm \
    --pipeline-v2 \
    -o /tmp/test-report.json
```

### Step 2: Extract Renames

```sh
# Extract all renames from the report
jq '[.changes[].breaking_api_changes[] | select(.change == "renamed")]' \
    /tmp/test-report.json > /tmp/renames.json
```

### Step 3: Verify True Renames Preserved

Check that all 15 true renames from Section 3 appear in the output. Key ones
to verify (low-similarity same-family renames most at risk):

```sh
jq '.[] | select(.symbol | test("usePageInsets|tertiaryNav|isSecondary|isActive"))' \
    /tmp/renames.json
```

### Step 4: Verify False Renames Eliminated

Check that the 28 false renames from Section 4 do NOT appear:

```sh
# Cross-family false renames (should be gone with threshold fix):
jq '.[] | select(.after | test("DrawerPanelDescription|SidebarBackgroundVariant|MenuToggleSize|ButtonState|DrawerContentColorVariant|ToolbarColorVariant|EmptyStateStatus"))' \
    /tmp/renames.json
```

### Step 5: Check for New False Positives

Manually review any renames where old and new symbols come from different
component directories. These are the most likely to be false positives.

### Step 6: Run Unit Tests

```sh
cargo test -p semver-analyzer-core -- rename
cargo test -p semver-analyzer-ts --lib
```

---

## 7. Known Gaps and Future Work

### Phase 1 (Low Risk) — Implemented

1. **Cross-family threshold**: Added `same_family` closure to `detect_renames()`.
   Applies 0.50 threshold for cross-family candidates (vs 0.15 for same-family).
   Fixes 10 of 11 cross-family false renames.

2. **Text → Content rule**: When emitting "removed, no replacement" rules,
   checks if a sibling in the same family was renamed. If `TextContent` →
   `Content` exists, annotates `Text`'s removal with "use Content instead."

### Phase 2 (Medium Risk) — Future Work

3. **Same-family false prop renames**: `isDisabled`→`hasAnimations`,
   `isOverflowLabel`→`isClickable`, `bodyAriaLabel`→`backdropClassName`. These
   need fingerprint disambiguation within same-component — cannot be fixed by
   similarity thresholds alone. Consider: when multiple boolean props are
   removed and multiple boolean props are added within the same interface,
   require higher similarity or additional structural context.

4. **TextListItemVariants → HelperTextItemVariant**: Cross-family with similarity
   0.714. The 0.50 threshold alone won't catch this. Needs additional guard:
   deprecated type_alias should not match non-related families, or require
   semantic naming pattern match.

5. **Many-to-one merges**: `Text` + `TextContent` both → `Content`. The 1:1
   greedy matching can't express this. The rule generator fix (Phase 1 item 2)
   handles the presentation, but the core algorithm could be enhanced to detect
   merge patterns explicitly.

6. **Composition inversion detection (SD pipeline)**: When a component migrates
   from deprecated to next-gen API (e.g., deprecated Select → next-gen Select),
   the SD pipeline should detect "composition inversions" — patterns where the
   old component managed a subcomponent internally (e.g., `<SelectToggle>`) but
   the new component exposes a render prop instead (e.g., `toggle={(ref) =>
   <MenuToggle ref={ref} />}`). This would produce migration rules mapping old
   internal-management props (`onToggle`, `toggleRef`, `toggleAriaLabel`) to
   the new render-function pattern. Requires:
   - Cross-component composition diffing between deprecated and non-deprecated
     versions
   - Detecting "render prop delegation" patterns
   - Mapping old internal component's props to new render prop's pattern

7. **Fuzzy prop matching in migration detection**: Extend `detect_migrations()`
   to run rename detection on unmatched props between deprecated and non-deprecated
   interfaces. Would catch mechanical renames like `selections` → `selected`
   (similarity 0.667) that currently fall into "removed with no equivalent."

8. **Fix-guidance data alignment**: The fix-guidance.yaml pipeline has a data
   corruption bug where rule IDs are mapped to wrong symbols (e.g., Text
   component rule IDs mapped to ModalProps descriptions). This is a generation
   pipeline bug, not a rename detector issue.
