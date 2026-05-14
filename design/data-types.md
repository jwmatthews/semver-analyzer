# Data Types Reference

Complete reference of every significant public struct and enum in the codebase.

## Core Types (`crates/core/src/types/`)

### API Surface

| Type | Fields | Notes |
|------|--------|-------|
| `ApiSurface<M>` | `symbols: Vec<Symbol<M>>` | Top-level API representation |
| `Symbol<M>` | name, qualified_name, kind, visibility, file, package, import_path, line, signature, extends, implements, is_abstract, is_readonly, is_static, accessor_kind, type_dependencies, members, language_data | Recursive: members are also `Symbol<M>` |
| `Signature` | parameters, return_type, type_parameters, is_async | Function/method signature |
| `Parameter` | name, type_annotation, optional, has_default, default_value, is_variadic | `is_variadic` (not `is_rest`) |
| `TypeParameter` | name, constraint, default | Generic type parameter |

### Enums

**`SymbolKind`** (15): Function, Method, Class, Struct, Interface, TypeAlias, Enum, EnumMember, Constant, Variable, Property, Constructor, GetAccessor, SetAccessor, Namespace

**`Visibility`** (5, ordered): Exported > Public > Protected > Internal > Private

**`AccessorKind`** (3): Get, Set, GetSet

**`FileStatus`** (4): Added, Modified, Deleted, Renamed

**`ApiChangeType`** (5): Removed, SignatureChanged, TypeChanged, VisibilityChanged, Renamed

**`ApiChangeKind`** (13): Function, Method, Class, Struct, Interface, Trait, TypeAlias, Constant, Enum, Constructor, Field, Property, ModuleExport

**`BehavioralChangeKind`** (4): Function, Method, Class, Module

**`TypeStatus`** (3): Modified, Removed, Added

### Structural Diff Output

| Type | Fields | Notes |
|------|--------|-------|
| `StructuralChange` | symbol, qualified_name, kind, package, change_type, before, after, description, is_breaking, impact, migration_target | Primary diff output |
| `StructuralChangeType` | Added(ChangeSubject), Removed(ChangeSubject), Changed(ChangeSubject), Renamed{from,to}, Relocated{from,to} | 5 lifecycle variants |
| `ChangeSubject` | Symbol, Member, Parameter, ReturnType, Visibility, Modifier, TypeParameter, BaseClass, InterfaceImpl, UnionValue | 10 variants: what changed |
| `MigrationTarget` | removed_symbol, removed_qualified_name, removed_package, replacement_symbol, replacement_qualified_name, replacement_package, matching_members, removed_only_members, overlap_ratio, old_extends, new_extends | Replacement detection |
| `MemberMapping` | old_name, new_name | Member-level rename within migration |

### Report Types

| Type | Fields | Notes |
|------|--------|-------|
| `AnalysisReport<L>` | repository, comparison, summary, changes, manifest_changes, added_files, packages, member_renames, inferred_rename_patterns, extensions, metadata | Top-level report |
| `Comparison` | from_ref, to_ref, from_sha, to_sha, commit_count, analysis_timestamp | Git comparison info |
| `Summary` | total_breaking_changes, breaking_api_changes, breaking_behavioral_changes, files_with_breaking_changes | Counts |
| `FileChanges<L>` | file, status, renamed_from, breaking_api_changes, breaking_behavioral_changes, container_changes | Per-file changes |
| `PackageChanges<L>` | name, old_version, new_version, type_summaries, constants, added_exports | Per-package view |
| `TypeSummary<L>` | name, definition_name, status, member_summary, removed_members, type_changes, migration_target, behavioral_changes, language_data, source_files | Per-component view |
| `MemberSummary` | total, removed, renamed, type_changed, added, removal_ratio | Member change counts |

### RemovalDisposition (4 variants)

```
MovedToRelatedType { target_type, mechanism }  // member absorbed by related component
ReplacedByMember { new_member }                // replaced by different member on same type
MadeAutomatic                                  // no longer needs explicit specification
TrulyRemoved                                   // genuinely gone, no replacement
```

### Bottom-Up Pipeline Types

| Type | Fields | Notes |
|------|--------|-------|
| `ChangedFunction` | qualified_name, name, file, line, kind, visibility, old_body, new_body, old_signature, new_signature | Git diff output |
| `FunctionSpec` | preconditions, postconditions, error_behavior, side_effects, notes | LLM-inferred behavioral spec |
| `BehavioralBreak<L>` | symbol, caused_by, call_path, evidence_description, confidence, description, category, evidence_type, is_internal_only | Behavioral change detection |
| `TestDiff` | test_file, removed_assertions, added_assertions, has_assertion_changes, full_diff | Test assertion changes |

**`EvidenceType`** (4): TestDelta, LlmAnalysis, BodyAnalysis, CallGraphPropagation

**`TestConvention`** (5): DotTest, DotSpec, TestsDir, Suffix(String), MirrorTree(String)

### Report Envelope

| Type | Fields | Notes |
|------|--------|-------|
| `ReportEnvelope` | language, version, summary, structural_changes, language_report | Two-tier container |
| `AnalysisSummary` | total_structural_breaking, total_structural_non_breaking, total_behavioral_changes, total_manifest_changes, packages_analyzed, files_changed, by_change_type | Counts by category |
| `ChangeTypeCounts` | added, removed, changed, renamed, relocated | Per-lifecycle counts |

---

## TypeScript Types (`crates/ts/`)

### Symbol Metadata

| Type | Fields | Notes |
|------|--------|-------|
| `TsSymbolData` | rendered_components, css | Per-symbol: what components it renders, what CSS tokens it uses |
| `TsReportData` | child_components, expected_children | Language-specific report extension |
| `TsAnalysisExtensions` | sd_result, hierarchy_deltas, new_hierarchies | SD pipeline output container |

**`TsCategory`** (8): DomStructure, CssClass, CssVariable, Accessibility, DefaultValue, LogicChange, DataAttribute, RenderOutput

**`TsManifestChangeType`** (10): EntryPointChanged, ExportsEntryRemoved, ExportsEntryAdded, ExportsConditionRemoved, ModuleSystemChanged, PeerDependencyAdded, PeerDependencyRemoved, PeerDependencyRangeChanged, EngineConstraintChanged, BinEntryRemoved

**`TsEvidence`** (4): TestDelta, JsxDiff, CssScan, LlmAnalysis

### Source Profile Types

| Type | Fields | Notes |
|------|--------|-------|
| `ComponentSourceProfile` | ~25 fields (see ts-crate.md) | Single-component profile |
| `TrackedAttributes<K>` | entries, unconditional | JSX attributes with conditionality |
| `RenderedComponent` | name, conditional | Component rendered in JSX |
| `CloneElementInjection` | injected_props | cloneElement prop injection |
| `ManagedAttributeBinding` | prop_name, generator_function, target_element, overridden_attributes, arg_position, component_overrides | Prop-to-HTML-attribute mapping |
| `BemStructure` | block, elements, modifiers, raw_tokens | CSS BEM structure |

### SD Pipeline Types

| Type | Fields | Notes |
|------|--------|-------|
| `SdPipelineResult` | ~20 fields (changes, trees, conformance, inventories, replacements) | Full SD output |
| `SourceLevelChange` | component, category, description, old_value, new_value, has_test_implications, test_description, element, migration_from, dependency_chain | Single source-level change |
| `CompositionTree` | root, family_members, edges | Component hierarchy tree |
| `CompositionEdge` | parent, child, relationship, required, bem_evidence, strength, prop_name | Parent-child edge |
| `CompositionChange` | family, change_type, description, before_pattern, after_pattern | Tree structural change |
| `ConformanceCheck` | family, check_type, description, correct_example | Nesting validation rule |
| `DeprecatedReplacement` | old_component, new_component, evidence_hosts, evidence_source | Deprecated -> replacement pair |
| `CssBlockProfile` | block, elements, has_containment, direct_child_nesting, descendant_nesting, sibling_relationships, layout_children | CSS BEM block structure |

**`EdgeStrength`** (4): Allowed, Structural, Wrapper, Required

**`ChildRelationship`** (6): BemElement, IndependentBlock, Internal, DirectChild, PropPassed, Unknown

**`SourceLevelCategory`** (15): DomStructure, AriaChange, RoleChange, DataAttribute, CssToken, PropDefault, PortalUsage, ContextDependency, Composition, ForwardRef, Memo, RenderedComponent, PropAttributeOverride, AttributeConditionality, PropDeprecated

**`CompositionChangeType`** (7): NewRequiredChild, PropToChild, ChildToProp, FamilyMemberRemoved, FamilyMemberAdded, PropDrivenToComposition, CompositionToPropDriven

**`ConformanceCheckType`** (4): MissingIntermediate, MissingChild, InvalidDirectChild, ExclusiveWrapper

---

## Java Types (`crates/java/`)

### Symbol Metadata

| Type | Fields | Notes |
|------|--------|-------|
| `JavaSymbolData` | annotations, throws, is_record, is_annotation_type, is_default, is_sealed, permits, is_final, is_non_sealed, is_synchronized, is_transient, is_volatile, is_native | Rich Java modifier set |
| `JavaAnnotation` | name, qualified_name, attributes | Single annotation |
| `JavaReportData` | source_level_changes, breaking_source_changes, annotation_changes, module_changes, serialization_issues | Counts |
| `JavaAnalysisExtensions` | sd_result | SD pipeline wrapper |

**`JavaCategory`** (8): LogicChange, ExceptionHandling, Configuration, Concurrency, DataAccess, Security, Serialization, Other

**`JavaManifestChangeType`** (8): DependencyAdded, DependencyRemoved, DependencyVersionChanged, ParentVersionChanged, PropertyChanged, PluginChanged, ProjectIdentityChanged, DependencyScopeChanged

### Java SD Types

| Type | Fields | Notes |
|------|--------|-------|
| `JavaClassProfile` | qualified_name, name, file, annotations, methods, constructor_params, implements, extends, is_final, is_sealed, is_abstract, permits, is_serializable, fields, module_name | Full class profile |
| `MethodProfile` | name, qualified_name, is_synchronized, is_native, is_override, is_default, is_abstract, thrown_exceptions, annotations, delegations, return_type, param_types | Method metadata |
| `FieldProfile` | name, field_type, is_transient, is_volatile, is_static, is_final | Field metadata |
| `JavaSourceChange` | class_name, category, description, old_value, new_value, is_breaking, method, dependency_chain | Source-level change |
| `JavaSdPipelineResult` | source_level_changes, old_profiles, new_profiles, module_changes, inheritance_summary | Full SD output |

**`JavaSourceCategory`** (22): AnnotationRemoved, AnnotationAdded, AnnotationChanged, DelegationChanged, ExceptionAdded, ExceptionRemoved, SynchronizationRemoved, SynchronizationAdded, SerializationFieldAdded, SerializationFieldRemoved, SerializationFieldTypeChanged, TransientChanged, OverrideRemoved, OverrideAdded, ConstructorDependencyChanged, ModuleExportRemoved, ModuleExportAdded, ModuleRequiresChanged, FinalAdded, FinalRemoved, SealedChanged, InheritanceChanged, NativeRemoved

---

## Konveyor Types (`crates/konveyor-core/`)

Most types are re-exported from the external `konveyor-core` crate. Key locally-defined types:

| Type | Fields | Notes |
|------|--------|-------|
| `RenamePatternEntry` | match_pattern, replace | Regex rename rule |
| `CssVarRenameEntry` | from, to | CSS variable rename |
| `CompositionRuleEntry` | child_pattern, parent, category, description, effort, package | Composition nesting rule |
| `PropRenameEntry` | old_prop, new_prop, components, package, description | Prop rename rule |
| `RenamePatternsFile` | All rule types loaded from YAML | Custom rules input |
| `RenamePatterns` | Compiled patterns with lookup methods | Runtime rule matching |
