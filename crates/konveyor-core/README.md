# semver-analyzer-konveyor-core

Language-independent shared library for [Konveyor](https://www.konveyor.io/) migration rule generation. Bridges `semver-analyzer-core` API diff types with `konveyor-core` rule definitions, providing the translation layer that converts raw semver breaking changes into actionable migration rules with fix strategies.

This crate is consumed by language-specific crates (e.g., `semver-analyzer-ts`) that add their own domain-specific rule generation on top.

## What It Does

1. **Translates** `ApiChange` objects from the diff engine into `KonveyorRule` / `FixStrategyEntry` / `KonveyorCondition` objects
2. **Consolidates** related rules by merging conditions and deduplicating
3. **Suppresses** redundant rules (e.g., prop-level rules covered by component-level deprecation)
4. **Writes** YAML output files (`ruleset.yaml`, `breaking-changes.yaml`, fix guidance)
5. **Loads** user-supplied configuration (rename patterns, composition rules, prop renames, etc.)

## Key Types

### From `konveyor-core` (re-exported)

| Type | Description |
|------|-------------|
| `KonveyorRule` | A single migration rule with ID, labels, effort, condition, message, and fix strategy |
| `KonveyorCondition` | When-condition: `FrontendReferenced`, `FileContent`, `Or`, `And`, `Json`, `FrontendPattern` |
| `KonveyorRuleset` | Ruleset metadata (name, description, labels) |
| `FixStrategyEntry` | Fix strategy with kind, from/to, component, prop, member mappings |
| `FixGuidanceDoc` | Top-level fix guidance document |
| `FrontendReferencedFields` | Condition fields: pattern, location, component, parent, value, from |

### Defined in this crate

| Type | Description |
|------|-------------|
| `RenamePatterns` | Compiled rename patterns with `load(path)` and `find_replacement(symbol)` |
| `RenamePatternsFile` | YAML config aggregating rename patterns, composition rules, prop renames, value reviews, missing imports, and component warnings |
| `ConstantGroupKey` | Grouping key for collapsible constant changes (package + change_type + strategy) |
| `CompoundToken` | Compound token with removed/added member suffixes |
| `PackageInfo` | Package name and version for monorepo packages |

## Key Functions

### Rule Construction

```rust
// Build a condition for detecting usage of a changed symbol
let condition = build_frontend_condition(&api_change, "Button", Some("@patternfly/react-core"));

// Build a regex pattern for a symbol
let pattern = build_pattern(&ApiChangeKind::Class, &ApiChangeType::Removed, "Button", &None);

// Map an API change to its fix strategy
let strategy = api_change_to_strategy(&change, &rename_patterns, &member_renames, "file.d.ts");

// Build a combined rule for a group of constant changes
let rule = build_combined_constant_rule(&key, &changes, &mut id_counts);
```

### Rule Optimization

```rust
// Consolidate related rules into combined rules
let (consolidated, id_map) = consolidate_rules(rules);

// Remove rules already covered by parent-level rules
let filtered = suppress_redundant_token_rules(rules, &covered_symbols);
let filtered = suppress_redundant_prop_rules(rules);
```

### Package Resolution

```rust
// Resolve npm package name from a file path
let pkg = resolve_npm_package("packages/react-core/src/Button.d.ts", &cache);
// -> Some("@patternfly/react-core")

// Read package.json at a git ref
let (name, version) = read_package_json_at_ref(repo, "v5.0.0", "packages/react-core/package.json");
```

### Output

```rust
// Write conformance rules to disk
write_conformance_rules(&output_dir, &rules)?;

// Write fix guidance
let fix_dir = write_fix_guidance_dir(&output_dir, &fix_guidance)?;
```

## User Configuration

The `RenamePatternsFile` YAML format supports:

```yaml
rename_patterns:
  - match_pattern: "^(\\w+)_Top$"
    replace: "${1}_BlockStart"

composition_rules:
  - child_pattern: "^ModalBody$"
    parent: "Modal"
    category: "mandatory"

prop_renames:
  - old_prop: "isFlat"
    new_prop: "variant"
    components: ["Card"]

value_reviews:
  - prop: "variant"
    component: "Button"
    value: "plain"

missing_imports:
  - has_pattern: "<DatePicker"
    missing_pattern: "import.*DatePicker"

component_warnings:
  - pattern: "^Toolbar$"
    description: "Toolbar DOM structure changed"
```

## Constants

| Constant | Value | Description |
|----------|-------|-------------|
| `CONSTANT_COLLAPSE_THRESHOLD` | 10 | Minimum constants from the same package/change-type before collapsing into a combined rule |

## Dependencies

| Crate | Purpose |
|-------|---------|
| `semver-analyzer-core` | Core API diff types (`ApiChange`, `ApiChangeKind`, etc.) |
| `konveyor-core` | Canonical Konveyor rule types shared with the frontend analyzer provider |
| `serde`, `serde_json`, `serde_yaml` | Serialization and YAML output |
| `anyhow` | Error handling |
| `regex` | Pattern building and matching |
| `chrono` | Timestamps |
| `tracing` | Structured logging |

## License

Apache-2.0
