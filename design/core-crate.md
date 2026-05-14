# Core Crate (`crates/core/`)

Language-agnostic foundation. Defines all shared types, the `Language` trait system, and the structural diff engine.

## File Map

```
crates/core/src/
  lib.rs              Re-exports, TestLang for unit tests
  traits.rs           Language, LanguageSemantics, BehaviorAnalyzer, HierarchySemantics, etc.
  shared.rs           SharedFindings (TD/BU cross-pipeline coordination)
  error.rs            ErrorTip trait, Diagnosed wrapper
  diagnostics.rs      DegradationTracker
  cli.rs              Shared clap argument structs
  git.rs              Git utilities (read_git_file, worktree management)
  types/
    mod.rs            Re-exports
    surface.rs        ApiSurface, Symbol, SymbolKind, Visibility, Signature, Parameter, etc.
    report.rs         AnalysisReport, StructuralChange, StructuralChangeType, MigrationTarget, etc.
    envelope.rs       ReportEnvelope (two-tier report container)
    bu.rs             ChangedFunction, FunctionSpec, BehavioralBreak, TestDiff, etc.
    change_subject.rs ChangeSubject enum (what aspect of a symbol changed)
  diff/
    mod.rs            6-phase diff orchestration, diff_surfaces entry points
    compare.rs        Symbol-to-symbol comparison (visibility, signatures, members, etc.)
    rename.rs         4-pass fingerprint-based rename detection
    relocate.rs       Relocation detection (deprecated/next path canonicalization)
    migration.rs      Migration detection (member overlap analysis)
    helpers.rs        Factory functions, kind labels, summaries
    tests.rs          ~40 unit tests with TsLikeTestSemantics
```

## Key Types

### `Symbol<M>` (`types/surface.rs`)

The fundamental unit of API surface. Generic over metadata type `M` (e.g., `TsSymbolData`, `JavaSymbolData`, or `()`).

```
Symbol<M>
  name: String
  qualified_name: String        // e.g., "ButtonProps.variant" or "com.example.MyClass.method"
  kind: SymbolKind              // 15 variants
  visibility: Visibility        // 5 variants: Exported > Public > Protected > Internal > Private
  file: PathBuf
  package: Option<String>
  import_path: Option<String>
  line: usize
  signature: Option<Signature>
  extends: Option<String>
  implements: Vec<String>
  is_abstract: bool
  is_readonly: bool
  is_static: bool
  accessor_kind: Option<AccessorKind>
  type_dependencies: Vec<String>
  members: Vec<Symbol<M>>       // Recursive: interfaces have property members, classes have methods
  language_data: M              // Language-specific metadata
```

### `SymbolKind` (15 variants)

`Function`, `Method`, `Class`, `Struct`, `Interface`, `TypeAlias`, `Enum`, `EnumMember`, `Constant`, `Variable`, `Property`, `Constructor`, `GetAccessor`, `SetAccessor`, `Namespace`

### `StructuralChangeType` (5 variants + `ChangeSubject`)

```
Added(ChangeSubject)     // New symbol/member/parameter appeared
Removed(ChangeSubject)   // Symbol/member/parameter removed
Changed(ChangeSubject)   // Type, visibility, modifier, etc. changed
Renamed { from, to }     // Symbol renamed (detected by fingerprint matching)
Relocated { from, to }   // Symbol moved (deprecated/next path changes)
```

### `ChangeSubject` (10 variants)

Describes what aspect of a symbol was affected:
`Symbol`, `Member`, `Parameter`, `ReturnType`, `Visibility`, `Modifier`, `TypeParameter`, `BaseClass`, `InterfaceImpl`, `UnionValue`

### `MigrationTarget`

When a removed interface/class has a detected replacement:
```
MigrationTarget
  removed_symbol, removed_qualified_name, removed_package
  replacement_symbol, replacement_qualified_name, replacement_package
  matching_members: Vec<MemberMapping>  // old_name -> new_name
  removed_only_members: Vec<String>     // members with no match
  overlap_ratio: f64                    // confidence metric
  old_extends, new_extends
```

## Diff Engine Internals

### Rename Detection Algorithm (`diff/rename.rs`)

The rename detector uses `MemberFingerprint` -- a coarse structural hash of a symbol:
```
MemberFingerprint { kind, return_type, is_optional, param_count }
```

Three fingerprint granularities:
1. **Exact**: literal types and names
2. **Normalized**: PascalCase -> `_T_`, param names -> `_p_`
3. **Deep normalized**: also string literals -> `_V_`

Each pass groups symbols by fingerprint, then picks the best match within each group by `name_similarity()` (LCS ratio). Thresholds prevent false positives:
- Same family (same directory): 0.15 (lenient, renames within a component are common)
- Cross family: 0.50 (strict, require substantial name overlap)
- Ambiguous groups (>2 candidates with primitive types): 0.45

**Cross-family sibling guard**: A cross-family rename is only accepted if there exists a confirmed sibling rename between the same pair of directories. This prevents spurious matches between unrelated components.

### Migration Detection (`diff/migration.rs`)

For each removed interface/class:
1. Search for candidates in the same family (same canonical directory)
2. Compute member overlap ratio
3. Adaptive thresholds scale with member count
4. Small-set fuzzy matching for near-miss prop names (0.60 similarity + type compatibility)
5. Keep best match (highest overlap ratio, minimum 0.25)

### Type Compatibility Check (`diff/compare.rs`)

`types_structurally_similar(old, new, primitives)` performs coarse shape comparison using `TypeCategory`:
- `Array`: ends with `[]`
- `Object`: contains `{` and `}`
- `Function`: contains `=>`
- `Tuple`: starts with `[`
- `Primitive`: in primitive set
- `Reference`: everything else

Same category = compatible. Different category = incompatible (blocks rename).

## SharedFindings (`shared.rs`)

Thread-safe coordination between TD and BU pipelines:

```
SharedFindings<L>
  structural_breaks: DashMap<String, StructuralChange>
  behavioral_breaks: DashMap<String, BehavioralBreak<L>>
  td_broadcast_tx: broadcast::Sender<String>      // capacity 4096
  old_surface: tokio::sync::OnceCell<Arc<ApiSurface>>
  new_surface: tokio::sync::OnceCell<Arc<ApiSurface>>
  degradation: Arc<DegradationTracker>
```

**Flow**: TD inserts structural breaks into DashMap and broadcasts qualified names. BU subscribes via `BuReceiver`, drains broadcast into a local `HashSet`, and checks both skip set and DashMap before analyzing each function.

## Error Handling

The `ErrorTip` trait pattern:
```rust
// Language crate defines error enum with tips:
impl ErrorTip for WorktreeError {
    fn tip(&self) -> Option<String> {
        match self {
            Self::RefNotFound { .. } => Some("Check that the git ref exists: git log --oneline <ref>".into()),
            // ...
        }
    }
}

// At the boundary, wrap the error:
worktree_operation().diagnose()?;  // attaches tip to anyhow chain

// CLI extracts and renders:
if let Some(diagnosed) = err.downcast_ref::<DiagnosedError>() {
    eprintln!("Tip: {}", diagnosed.tip);
}
```

`DegradationTracker` records non-fatal issues during analysis (LLM timeouts, parse failures) and prints a summary at the end.
