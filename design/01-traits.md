# Core Traits

Source of truth: `crates/core/src/traits.rs`

---

## 1. `Language` trait (line 608)

The central integration point for multi-language support. Composes
`LanguageSemantics<Self::SymbolData> + MessageFormatter + Send + Sync + 'static`.

```rust
pub trait Language:
    LanguageSemantics<Self::SymbolData> + MessageFormatter + Send + Sync + 'static
{
    type SymbolData: Debug + Clone + Default + PartialEq + Eq + Serialize + DeserializeOwned + Send + Sync;
    type Category: Debug + Clone + Serialize + DeserializeOwned + Eq + Hash + Send + Sync;
    type ManifestChangeType: Debug + Clone + Serialize + DeserializeOwned + Eq + PartialEq + Send + Sync;
    type Evidence: Debug + Clone + Serialize + DeserializeOwned + Send + Sync;
    type ReportData: Debug + Clone + Serialize + DeserializeOwned + Send + Sync;
    type AnalysisExtensions: Debug + Clone + Default + Serialize + DeserializeOwned + Send + Sync;
    // ...
}
```

### Associated types (6)

| Type | Bounds | Purpose |
|------|--------|---------|
| `SymbolData` | `Debug + Clone + Default + PartialEq + Eq + Serialize + DeserializeOwned + Send + Sync` | Per-symbol metadata in `Symbol<M>.language_data`. TS: `TsSymbolData`. |
| `Category` | `Debug + Clone + Serialize + DeserializeOwned + Eq + Hash + Send + Sync` | Behavioral change categories. TS: `TsCategory`. |
| `ManifestChangeType` | `Debug + Clone + Serialize + DeserializeOwned + Eq + PartialEq + Send + Sync` | Manifest change variants. TS: npm changes. |
| `Evidence` | `Debug + Clone + Serialize + DeserializeOwned + Send + Sync` | Evidence data on behavioral changes. |
| `ReportData` | `Debug + Clone + Serialize + DeserializeOwned + Send + Sync` | Language-specific report data. |
| `AnalysisExtensions` | `Debug + Clone + Default + Serialize + DeserializeOwned + Send + Sync` | Pipeline results (SD, hierarchy deltas). TS: `TsAnalysisExtensions`. |

### Constants (4)

| Constant | Type | Line | Purpose |
|----------|------|------|---------|
| `NAME` | `&'static str` | 663 | Language identifier for serialization dispatch. |
| `MANIFEST_FILES` | `&'static [&'static str]` | 675 | Manifest file paths (e.g., `["package.json"]`). |
| `SOURCE_FILE_PATTERNS` | `&'static [&'static str]` | 696 | Glob patterns for source files (e.g., `["*.ts", "*.tsx"]`). |
| `RENAMEABLE_SYMBOL_KINDS` | `&'static [SymbolKind]` | 660 | Symbol kinds eligible for rename inference. |

### Methods (20)

| Method | Required/Default | Line | Description |
|--------|-----------------|------|-------------|
| `discover_package_manifests` | Default | 685 | Discover per-package manifests in monorepos; returns `(path, name)` pairs. |
| `extract` | Required | 709 | Extract public API surface from source at a git ref. |
| `extract_keeping_worktree` | Default | 726 | Extract API surface and optionally return a live worktree handle. |
| `parse_changed_functions` | Required | 738 | Parse diff between two refs; identify functions whose bodies changed. |
| `find_callers` | Required | 746 | Find callers of a symbol in a file. |
| `find_references` | Required | 749 | Find all references to a public symbol across the project. |
| `find_tests` | Required | 752 | Find test files associated with a source file. |
| `diff_test_assertions` | Required | 755 | Diff a test file between two refs; return changed assertions. |
| `diff_manifest_content` | Required | 772 | Diff manifest content between two versions (static method, `Self: Sized`). |
| `should_exclude_from_analysis` | Required | 783 | Whether a file path should be excluded from BU analysis (static method). |
| `build_report` | Required | 793 | Build the language-specific report from analysis results (`Self: Sized`). |
| `behavioral_change_kind` | Default | 808 | Map evidence type to behavioral change kind. Default: `Function`. |
| `extract_referenced_symbols` | Default | 815 | Extract symbol references from a behavioral change description. Default: empty. |
| `display_name` | Default | 822 | Format a qualified name for display. Default: identity. |
| `llm_categories` | Default | 834 | Return behavioral categories for LLM prompts. Default: empty. |
| `run_extended_analysis` | Default | 851 | Run language-specific extended analysis (e.g., SD pipeline). Default: empty extensions. |
| `finalize_extensions` | Default | 869 | Post-process extensions after TD+SD complete. Default: no-op. |
| `extensions_log_summary` | Default | 884 | Return log-friendly summary lines for extended analysis. Default: empty. |

Required: 8. Default: 10 (including 2 static defaults: `discover_package_manifests`, `extract_keeping_worktree` which calls `extract`).

---

## 2. `LanguageSemantics<M>` trait (line 98)

Semantic rules consumed by the diff engine. Generic parameter
`M: Default + Clone + PartialEq = ()` defaults to unit, allowing
`&dyn LanguageSemantics` usage without generics.

```rust
pub trait LanguageSemantics<M: Default + Clone + PartialEq = ()> {
    // ...
}
```

### Methods (19)

| Method | Required/Default | Line | Description |
|--------|-----------------|------|-------------|
| `is_member_addition_breaking` | Required | 107 | Whether adding a member to a container is breaking. |
| `same_family` | Required | 118 | Whether two symbols belong to the same logical family/group. |
| `same_identity` | Required | 130 | Whether two symbols are the same concept at different paths. |
| `visibility_rank` | Required | 139 | Numeric rank for a visibility level (higher = more visible). |
| `parse_union_values` | Default | 146 | Parse union/constrained type values for fine-grained diffing. Default: `None`. |
| `is_async_wrapper` | Default | 159 | Whether a return type string represents an async wrapper. Default: `false`. |
| `format_import_change` | Default | 171 | Format an import statement change hint. Default: generic format. |
| `should_skip_symbol` | Default | 186 | Whether a symbol should be excluded from diff analysis. Default: `false`. |
| `member_label` | Default | 195 | Human-readable label for members. Default: `"members"`. |
| `extract_rename_fallback_key` | Default | 208 | Fallback key for rename matching from symbol metadata. Default: `None`. |
| `canonical_name_for_relocation` | Default | 221 | Normalize a qualified name for relocation detection. Default: identity. |
| `classify_relocation` | Default | 233 | Classify a relocation direction. Default: `None`. |
| `derive_import_subpath` | Default | 245 | Derive import subpath for migration descriptions. Default: package name. |
| `diff_language_data` | Default | 257 | Produce additional structural changes from language-specific metadata. Default: empty. |
| `post_process` | Default | 265 | Post-process change list before returning. Default: no-op. |
| `hierarchy` | Default | 272 | Return optional `HierarchySemantics` implementation. Default: `None`. |
| `renames` | Default | 282 | Return optional `RenameSemantics` implementation. Default: `None`. |
| `body_analyzer` | Default | 292 | Return optional `BodyAnalysisSemantics` implementation. Default: `None`. |
| `primitive_type_names` | Default | 308 | Primitive type names for structural similarity. Default: `["string", "number", "boolean", "void", "null"]`. |

Required: 4. Default: 15.

---

## 3. `BehaviorAnalyzer` trait (line 39)

Language-agnostic LLM-based behavioral analysis. Used by the BU pipeline.
Implementations may use direct LLM APIs, `goose run`, `opencode run`, or
any agent CLI via `--llm-command`.

### Methods (4)

| Method | Required/Default | Line | Description |
|--------|-----------------|------|-------------|
| `infer_spec` | Required | 44 | Infer a function's behavioral spec from body and signature alone. |
| `infer_spec_with_test_context` | Required | 50 | Infer a spec with additional test file context (reduces hallucination). |
| `specs_are_breaking` | Required | 62 | Compare two specs; determine if change is breaking (Tier 1 structural, Tier 2 LLM fallback). |
| `check_propagation` | Required | 79 | Check whether a caller propagates a behavioral break from a callee. Returns `true` if break propagates. |

All 4 required.

---

## 4. `HierarchySemantics<M>` trait (line 332)

Component hierarchy inference for languages with composition models
(React, Vue, Django templates). Accessed via `LanguageSemantics::hierarchy()`.
Generic parameter `M: Default + Clone + PartialEq = ()`.

```rust
pub trait HierarchySemantics<M: Default + Clone + PartialEq = ()> {
    // ...
}
```

### Methods (7)

| Method | Required/Default | Line | Description |
|--------|-----------------|------|-------------|
| `family_source_paths` | Required | 337 | Get file paths belonging to a component family directory. |
| `family_name_from_symbols` | Required | 343 | Get a human-readable family name from a group of symbols. |
| `cross_family_relationships` | Required | 349 | Detect cross-family relationships (e.g., React context imports). Returns `(consumer, provider, relationship)` triples. |
| `related_family_content` | Required | 360 | Read related component signatures for cross-family context in LLM prompts. |
| `is_hierarchy_candidate` | Required | 375 | Whether a symbol is a candidate for hierarchy inference. |
| `min_components_for_hierarchy` | Default | 379 | Minimum exported types for hierarchy inference. Default: `2`. |
| `compute_deterministic_hierarchy` | Default | 391 | Compute hierarchy deterministically. Default: empty map. |

Required: 5. Default: 2.

---

## 5. `RenameSemantics` trait (line 409)

Data preparation for LLM-based rename inference (e.g., CSS physical-to-logical
property renames). Accessed via `LanguageSemantics::renames()`.

### Methods (4)

| Method | Required/Default | Line | Description |
|--------|-----------------|------|-------------|
| `sample_removed_constants` | Default | 414 | Sample removed constants for rename pattern inference. Default: first 30. |
| `sample_added_constants` | Default | 425 | Sample added constants for rename pattern inference. Default: first 30. |
| `min_removed_for_constant_inference` | Default | 431 | Minimum removed constants to trigger rename inference. Default: `50`. |
| `min_removed_for_interface_inference` | Default | 437 | Minimum removed interfaces to trigger interface rename inference. Default: `2`. |

All 4 default.

---

## 6. `BodyAnalysisSemantics` trait (line 454)

Deterministic body-level analysis for behavioral change detection without
LLM assistance. Accessed via `LanguageSemantics::body_analyzer()`.

### Methods (1)

| Method | Required/Default | Line | Description |
|--------|-----------------|------|-------------|
| `analyze_changed_body` | Required | 464 | Run deterministic analysis on a changed function body. Returns `Vec<BodyAnalysisResult>`. |

---

## 7. `MessageFormatter` trait (line 478)

Language-specific human-readable descriptions for changes. Each language
owns its messaging entirely; no generic template in core.

### Methods (1)

| Method | Required/Default | Line | Description |
|--------|-----------------|------|-------------|
| `describe` | Required | 480 | Produce a human-readable description for a `StructuralChange`. |

---

## 8. `WorktreeAccess` trait (line 493)

Opaque handle to a checked-out worktree. Keeps the worktree alive as long
as the handle exists. Supertrait bounds: `Send + Sync + 'static`.

### Methods (1)

| Method | Required/Default | Line | Description |
|--------|-----------------|------|-------------|
| `path` | Required | 495 | Filesystem path to the worktree directory. |

---

## 9. Supporting types

### `ExtendedAnalysisParams` (line 514)

Parameters for `Language::run_extended_analysis`.

| Field | Type | Description |
|-------|------|-------------|
| `repo` | `PathBuf` | Path to the primary repository. |
| `from_ref` | `String` | Git ref for the old version. |
| `to_ref` | `String` | Git ref for the new version. |
| `dep_dir` | `Option<PathBuf>` | Optional dependency resource repository path. |
| `removed_dep_components` | `Vec<String>` | Component directories removed between dep-repo versions. |
| `removed_dep_entry_files` | `Vec<String>` | Top-level SCSS/CSS entry point files removed. |
| `dep_repo_packages` | `HashMap<String, String>` | Dependency repo packages (name to version at new ref). |
| `from_worktree_path` | `Option<PathBuf>` | Filesystem path to the from-ref worktree (shared by TD). |
| `to_worktree_path` | `Option<PathBuf>` | Filesystem path to the to-ref worktree (shared by TD). |
| `dead_css_classes_after_swap` | `Vec<(String, String)>` | CSS classes where naive prefix swap produces a dead class. |
| `old_css_class_inventory` | `HashSet<String>` | Full CSS class inventory from the old dep repo version. |
| `new_css_class_inventory` | `HashSet<String>` | Full CSS class inventory from the new dep repo version. |

### `LlmCategoryDefinition` (line 581)

A behavioral change category definition for LLM prompts.

| Field | Type | Description |
|-------|------|-------------|
| `id` | `String` | Machine-readable identifier matching serde name of `Language::Category` variant. |
| `label` | `String` | Short human label (e.g., `"DOM/render changes"`). |
| `description` | `String` | Detailed description for the LLM prompt. |

### `ExtractionWithWorktree<M>` (line 499)

Type alias: `(ApiSurface<M>, Option<Arc<dyn WorktreeAccess>>)`.
Result of `extract_keeping_worktree`.

### Free functions

| Function | Line | Description |
|----------|------|-------------|
| `diff_surfaces_with_semantics<M, S>` | 895 | Primary TD entry point. Compares two `ApiSurface<M>` using a `LanguageSemantics<M>`. |
| `diff_surfaces<M>` | 912 | Compare two surfaces using `MinimalSemantics` (no language-specific rules). |

---

## 10. Trait relationship diagram

```
                          ┌──────────────────────────────────────┐
                          │            Language                   │
                          │                                      │
                          │  6 associated types                  │
                          │  4 constants                         │
                          │  20 methods (8 required, 10 default, │
                          │             2 static defaults)       │
                          └──────────┬───────────────────────────┘
                                     │ supertrait of
                          ┌──────────┴──────────┐
                          │                     │
              ┌───────────┴──────────┐  ┌───────┴──────────┐
              │  LanguageSemantics   │  │ MessageFormatter  │
              │  <Self::SymbolData>  │  │                   │
              │                      │  │  1 method:        │
              │  19 methods          │  │   describe()      │
              │  (4 required,        │  └───────────────────┘
              │   15 default)        │
              │                      │
              │  Optional accessors: │
              │   hierarchy()  ──────┼──► HierarchySemantics<M>  (7 methods)
              │   renames()    ──────┼──► RenameSemantics         (4 methods)
              │   body_analyzer() ──┼──► BodyAnalysisSemantics   (1 method)
              └──────────────────────┘

  Separate (not supertrait):

  ┌──────────────────────┐    ┌──────────────────┐
  │  BehaviorAnalyzer    │    │  WorktreeAccess   │
  │  (BU pipeline, LLM)  │    │  (Send+Sync)      │
  │  4 methods            │    │  1 method: path() │
  └──────────────────────┘    └──────────────────┘
```

`Language` composes `LanguageSemantics` and `MessageFormatter` as supertraits.
`BehaviorAnalyzer` and `WorktreeAccess` are independent traits, not part of
the `Language` hierarchy. The three optional capability traits
(`HierarchySemantics`, `RenameSemantics`, `BodyAnalysisSemantics`) are accessed
via accessor methods on `LanguageSemantics`, not via supertrait bounds.
