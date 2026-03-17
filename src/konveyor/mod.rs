//! Konveyor rule generation from semver-analyzer breaking change reports.
//!
//! Transforms an `AnalysisReport` into a Konveyor-compatible ruleset directory
//! that can be consumed by `konveyor-analyzer --rules <dir>`.
//!
//! The mapping is deterministic: each breaking change type produces a specific
//! rule pattern using `builtin.filecontent` (regex) or `builtin.json` (xpath).

use std::collections::{BTreeMap, BTreeSet, HashMap};
use std::path::Path;

use anyhow::{Context, Result};
use serde::{Deserialize, Serialize};

use semver_analyzer_core::{
    AnalysisReport, ApiChange, ApiChangeKind, ApiChangeType, BehavioralChange, FileChanges,
    ManifestChange, ManifestChangeType,
};

// ── User-supplied rename patterns ───────────────────────────────────────

/// A single regex-based rename pattern.
///
/// When a symbol is removed and its name matches `match_regex`, the
/// replacement is computed by applying the regex substitution `replace`.
/// Standard regex capture groups (`$1`, `${1}`) are supported.
#[derive(Debug, Clone, Deserialize)]
pub struct RenamePatternEntry {
    /// Regex to match against the removed symbol name.
    #[serde(rename = "match")]
    pub match_pattern: String,
    /// Replacement string (supports `$1`, `${1}` capture group references).
    pub replace: String,
}

/// A composition rule: detect a child component inside a parent component.
///
/// Generates rules with the `parent` constraint on `frontend.referenced`.
#[derive(Debug, Clone, Deserialize)]
pub struct CompositionRuleEntry {
    /// Regex pattern for the child component (e.g., `"Icon$"`).
    pub child_pattern: String,
    /// Regex for the required parent component (e.g., `"^Button$"`).
    pub parent: String,
    /// Rule category: `mandatory` or `potential`.
    #[serde(default = "default_mandatory")]
    pub category: String,
    /// Human-readable description.
    pub description: String,
    /// Effort estimate.
    #[serde(default = "default_effort_2")]
    pub effort: u32,
    /// Optional package scope (e.g., `@patternfly/react-core`).
    #[serde(default)]
    pub package: Option<String>,
}

/// A prop rename rule: detect usage of an old prop name on specific components.
#[derive(Debug, Clone, Deserialize)]
pub struct PropRenameEntry {
    /// Old prop name.
    pub old_prop: String,
    /// New prop name (for message/fix guidance).
    pub new_prop: String,
    /// Regex matching the components this rename applies to.
    pub components: String,
    /// Package scope.
    #[serde(default)]
    pub package: Option<String>,
    /// Human-readable description.
    #[serde(default)]
    pub description: Option<String>,
}

/// A component warning: emit a JSX_COMPONENT rule for a component whose internal
/// DOM/CSS rendering changed without an API surface change.
///
/// These are informational rules that alert consumers to review usages of a
/// component whose behavior changed internally.
#[derive(Debug, Clone, Deserialize)]
pub struct ComponentWarningEntry {
    /// Regex pattern matching the component name (e.g., `"^TextArea$"`).
    pub pattern: String,
    /// Package scope.
    #[serde(default)]
    pub package: Option<String>,
    /// Rule category: `mandatory` or `potential`.
    #[serde(default = "default_potential")]
    pub category: String,
    /// Human-readable description.
    pub description: String,
    /// Effort estimate.
    #[serde(default = "default_effort_1")]
    pub effort: u32,
}

/// A missing co-requisite import rule: flag when pattern A is present but pattern B is absent.
///
/// Uses `and` + `not` combinators with `builtin.filecontent` to detect cases
/// where a file has one import but is missing a newly required companion import.
#[derive(Debug, Clone, Deserialize)]
pub struct MissingImportEntry {
    /// Regex that must be present in the file (the existing import).
    pub has_pattern: String,
    /// Regex that must be absent from the file (the missing import).
    pub missing_pattern: String,
    /// File glob pattern (e.g., `"\\.(ts|tsx|js|jsx)$"`).
    #[serde(default = "default_ts_file_pattern")]
    pub file_pattern: String,
    /// Rule category: `mandatory` or `potential`.
    #[serde(default = "default_mandatory")]
    pub category: String,
    /// Human-readable description.
    pub description: String,
    /// Effort estimate.
    #[serde(default = "default_effort_1")]
    pub effort: u32,
}

fn default_ts_file_pattern() -> String {
    r"\.(ts|tsx|js|jsx)$".to_string()
}

/// A value review rule: detect a specific prop value that may need updating.
///
/// Used for cases where a prop value is technically still valid but may need
/// review (e.g., `variant="plain"` on MenuToggle).
#[derive(Debug, Clone, Deserialize)]
pub struct ValueReviewEntry {
    /// Prop name.
    pub prop: String,
    /// Regex matching the component.
    pub component: String,
    /// Regex matching the value to flag.
    pub value: String,
    /// Package scope.
    #[serde(default)]
    pub package: Option<String>,
    /// Rule category: `mandatory` or `potential`.
    #[serde(default = "default_potential")]
    pub category: String,
    /// Human-readable description.
    pub description: String,
    /// Effort estimate.
    #[serde(default = "default_effort_1")]
    pub effort: u32,
}

fn default_mandatory() -> String {
    "mandatory".to_string()
}
fn default_potential() -> String {
    "potential".to_string()
}
fn default_effort_1() -> u32 {
    1
}
fn default_effort_2() -> u32 {
    2
}

/// Parsed rename patterns file (extended with composition rules, prop renames,
/// value review rules, missing import rules, and component warnings).
#[derive(Debug, Clone, Deserialize)]
pub struct RenamePatternsFile {
    #[serde(default)]
    pub rename_patterns: Vec<RenamePatternEntry>,
    #[serde(default)]
    pub composition_rules: Vec<CompositionRuleEntry>,
    #[serde(default)]
    pub prop_renames: Vec<PropRenameEntry>,
    #[serde(default)]
    pub value_reviews: Vec<ValueReviewEntry>,
    #[serde(default)]
    pub missing_imports: Vec<MissingImportEntry>,
    #[serde(default)]
    pub component_warnings: Vec<ComponentWarningEntry>,
}

/// Compiled rename patterns ready for matching.
#[derive(Debug, Clone)]
pub struct RenamePatterns {
    patterns: Vec<(regex::Regex, String)>,
    pub composition_rules: Vec<CompositionRuleEntry>,
    pub prop_renames: Vec<PropRenameEntry>,
    pub value_reviews: Vec<ValueReviewEntry>,
    pub missing_imports: Vec<MissingImportEntry>,
    pub component_warnings: Vec<ComponentWarningEntry>,
}

impl RenamePatterns {
    /// Load and compile rename patterns from a YAML file.
    pub fn load(path: &Path) -> Result<Self> {
        let content = std::fs::read_to_string(path)
            .with_context(|| format!("Failed to read rename patterns from {}", path.display()))?;
        let file: RenamePatternsFile = serde_yaml::from_str(&content).with_context(|| {
            format!("Failed to parse {} as rename patterns YAML", path.display())
        })?;

        let mut patterns = Vec::new();
        for entry in &file.rename_patterns {
            let re = regex::Regex::new(&entry.match_pattern).with_context(|| {
                format!("Invalid regex in rename pattern: {}", entry.match_pattern)
            })?;
            patterns.push((re, entry.replace.clone()));
        }

        eprintln!(
            "Loaded {} rename patterns from {}",
            patterns.len(),
            path.display()
        );
        if !file.composition_rules.is_empty() {
            eprintln!("Loaded {} composition rules", file.composition_rules.len());
        }
        if !file.prop_renames.is_empty() {
            eprintln!("Loaded {} prop renames", file.prop_renames.len());
        }
        if !file.value_reviews.is_empty() {
            eprintln!("Loaded {} value reviews", file.value_reviews.len());
        }
        if !file.missing_imports.is_empty() {
            eprintln!("Loaded {} missing import rules", file.missing_imports.len());
        }
        if !file.component_warnings.is_empty() {
            eprintln!(
                "Loaded {} component warnings",
                file.component_warnings.len()
            );
        }
        Ok(Self {
            patterns,
            composition_rules: file.composition_rules,
            prop_renames: file.prop_renames,
            value_reviews: file.value_reviews,
            missing_imports: file.missing_imports,
            component_warnings: file.component_warnings,
        })
    }

    /// Try to find a replacement for a removed symbol name.
    ///
    /// Returns `Some(new_name)` if any pattern matches, `None` otherwise.
    pub fn find_replacement(&self, symbol_name: &str) -> Option<String> {
        for (re, replace) in &self.patterns {
            if re.is_match(symbol_name) {
                let result = re.replace(symbol_name, replace.as_str()).to_string();
                if result != symbol_name {
                    return Some(result);
                }
            }
        }
        None
    }

    /// Empty patterns (no-op).
    pub fn empty() -> Self {
        Self {
            patterns: Vec::new(),
            composition_rules: Vec::new(),
            prop_renames: Vec::new(),
            value_reviews: Vec::new(),
            missing_imports: Vec::new(),
            component_warnings: Vec::new(),
        }
    }

    pub fn is_empty(&self) -> bool {
        self.patterns.is_empty()
    }
}

// ── Konveyor YAML types ─────────────────────────────────────────────────

/// Ruleset metadata (written to `ruleset.yaml`).
#[derive(Debug, Serialize)]
pub struct KonveyorRuleset {
    pub name: String,
    pub description: String,
    pub labels: Vec<String>,
}

/// A single Konveyor rule.
#[derive(Debug, Serialize)]
pub struct KonveyorRule {
    #[serde(rename = "ruleID")]
    pub rule_id: String,
    pub labels: Vec<String>,
    pub effort: u32,
    pub category: String,
    pub description: String,
    pub message: String,
    #[serde(skip_serializing_if = "Vec::is_empty")]
    pub links: Vec<KonveyorLink>,
    pub when: KonveyorCondition,
}

/// A hyperlink attached to a rule.
#[derive(Debug, Serialize)]
pub struct KonveyorLink {
    pub url: String,
    pub title: String,
}

/// A Konveyor `when` condition.
///
/// Supports `builtin.filecontent` (regex), `builtin.json` (xpath),
/// `frontend.referenced` (AST-level, requires the frontend-analyzer-provider),
/// and `or` (disjunction of conditions).
#[derive(Debug, Serialize)]
#[serde(untagged)]
pub enum KonveyorCondition {
    FileContent {
        #[serde(rename = "builtin.filecontent")]
        filecontent: FileContentFields,
    },
    Json {
        #[serde(rename = "builtin.json")]
        json: JsonFields,
    },
    FrontendReferenced {
        #[serde(rename = "frontend.referenced")]
        referenced: FrontendReferencedFields,
    },
    FrontendCssClass {
        #[serde(rename = "frontend.cssclass")]
        cssclass: FrontendPatternFields,
    },
    FrontendCssVar {
        #[serde(rename = "frontend.cssvar")]
        cssvar: FrontendPatternFields,
    },
    Or {
        or: Vec<KonveyorCondition>,
    },
    And {
        and: Vec<KonveyorCondition>,
    },
    /// Negated `builtin.filecontent`: matches when the pattern is NOT found.
    /// Serializes as `{ "not": true, "builtin.filecontent": { ... } }`.
    FileContentNegated {
        #[serde(rename = "not")]
        negated: bool,
        #[serde(rename = "builtin.filecontent")]
        filecontent: FileContentFields,
    },
}

/// Fields for `frontend.cssclass` and `frontend.cssvar` conditions.
#[derive(Debug, Serialize)]
pub struct FrontendPatternFields {
    pub pattern: String,
}

/// Fields for a `builtin.filecontent` condition.
#[derive(Debug, Serialize)]
pub struct FileContentFields {
    pub pattern: String,
    #[serde(rename = "filePattern")]
    pub file_pattern: String,
}

/// Fields for a `builtin.json` condition.
#[derive(Debug, Serialize)]
pub struct JsonFields {
    pub xpath: String,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub filepaths: Option<String>,
}

/// Fields for a `frontend.referenced` condition.
///
/// This condition requires the frontend-analyzer-provider gRPC server.
/// It performs AST-level symbol matching with location discriminators.
#[derive(Debug, Serialize)]
pub struct FrontendReferencedFields {
    /// Regex pattern for the symbol name.
    pub pattern: String,
    /// Where to look: IMPORT, JSX_COMPONENT, JSX_PROP, FUNCTION_CALL, TYPE_REFERENCE.
    pub location: String,
    /// Filter JSX props to only those on this component (regex).
    #[serde(skip_serializing_if = "Option::is_none")]
    pub component: Option<String>,
    /// Filter JSX components to only those inside this parent (regex).
    #[serde(skip_serializing_if = "Option::is_none")]
    pub parent: Option<String>,
    /// Filter JSX prop values to only those matching this regex.
    /// Used for prop value changes (e.g., `variant="tertiary"`).
    #[serde(skip_serializing_if = "Option::is_none")]
    pub value: Option<String>,
    /// Scope to imports from a specific package (e.g., `@patternfly/react-tokens`).
    #[serde(skip_serializing_if = "Option::is_none")]
    pub from: Option<String>,
}

// NOTE: All API and behavioral rules now use `frontend.referenced` conditions
// with package-scoped `from:` fields. The `builtin.filecontent` and `builtin.json`
// condition types are retained only for manifest rules and consolidated token groups
// where no package scope is available.

// ── Fix guidance types ──────────────────────────────────────────────────

/// How to fix a detected issue.
///
/// Mirrors the frontend-analyzer-provider's fix engine: each rule is mapped
/// to a deterministic fix strategy with confidence level.
#[derive(Debug, Clone, Serialize)]
pub struct FixGuidanceEntry {
    /// The rule ID this fix corresponds to.
    #[serde(rename = "ruleID")]
    pub rule_id: String,

    /// The fix strategy to apply.
    pub strategy: FixStrategy,

    /// How confident we are this fix is correct.
    pub confidence: FixConfidence,

    /// Where this fix guidance came from.
    pub source: FixSource,

    /// The affected symbol.
    pub symbol: String,

    /// Source file where the breaking change originates.
    pub file: String,

    /// Concrete instructions for fixing the issue.
    pub fix_description: String,

    /// Example of the old code pattern (when available).
    #[serde(skip_serializing_if = "Option::is_none")]
    pub before: Option<String>,

    /// Example of the new code pattern (when available).
    #[serde(skip_serializing_if = "Option::is_none")]
    pub after: Option<String>,

    /// Search pattern to find code that needs fixing.
    pub search_pattern: String,

    /// Suggested replacement (for mechanical fixes).
    #[serde(skip_serializing_if = "Option::is_none")]
    pub replacement: Option<String>,
}

/// What kind of fix to apply.
#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "snake_case")]
pub enum FixStrategy {
    /// Find-and-replace: rename old symbol to new symbol.
    Rename,
    /// Update function call sites to match new signature.
    UpdateSignature,
    /// Update type annotations to match new types.
    UpdateType,
    /// Remove usages of a deleted symbol and find alternatives.
    FindAlternative,
    /// Remove a property/field that no longer exists.
    RemoveUsage,
    /// Update import paths or module system (require ↔ import).
    UpdateImport,
    /// Update package.json dependency configuration.
    UpdateDependency,
    /// Requires manual review — behavioral change or complex refactor.
    ManualReview,
}

/// How confident the fix guidance is.
#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "snake_case")]
pub enum FixConfidence {
    /// Mechanical rename or direct replacement — safe to auto-apply.
    Exact,
    /// Pattern-based fix — likely correct but may need review.
    High,
    /// Inferred fix — needs human verification.
    Medium,
    /// Best-effort suggestion — may not be applicable.
    Low,
}

/// Where the fix guidance originates.
#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "snake_case")]
pub enum FixSource {
    /// Deterministic — derived from structural analysis.
    Pattern,
    /// AI-generated — from LLM behavioral analysis.
    Llm,
    /// Flagged for manual intervention.
    Manual,
}

/// Top-level fix guidance document written to `fix-guidance.yaml`.
#[derive(Debug, Serialize)]
pub struct FixGuidanceDoc {
    /// Version range this guidance applies to.
    pub migration: MigrationInfo,
    /// Summary statistics.
    pub summary: FixSummary,
    /// Per-rule fix entries.
    pub fixes: Vec<FixGuidanceEntry>,
}

/// Migration metadata.
#[derive(Debug, Serialize)]
pub struct MigrationInfo {
    pub from_ref: String,
    pub to_ref: String,
    pub generated_by: String,
}

/// Summary of fix guidance.
#[derive(Debug, Serialize)]
pub struct FixSummary {
    pub total_fixes: usize,
    pub auto_fixable: usize,
    pub needs_review: usize,
    pub manual_only: usize,
}

// ── Public API ───────────────────────────────────────────────────────────

/// Generate Konveyor rules from an `AnalysisReport`.
///
/// Each breaking API change, behavioral change, and manifest change
/// produces one rule. The mapping is fully deterministic.
///
/// When `provider` is `Frontend`, API change rules use `frontend.referenced`
/// conditions with AST-level location discriminators (JSX_COMPONENT, JSX_PROP,
/// IMPORT, etc.). When `Builtin`, rules use `builtin.filecontent` regex patterns.
pub fn generate_rules(
    report: &AnalysisReport,
    file_pattern: &str,
    pkg_cache: &HashMap<String, String>,
    rename_patterns: &RenamePatterns,
) -> Vec<KonveyorRule> {
    let mut rules = Vec::new();
    let mut id_counts: HashMap<String, usize> = HashMap::new();
    // API changes (per-file)
    for file_changes in &report.changes {
        // resolve_npm_package already appends /deprecated or /next when the
        // source file lives under those directories.  This ensures rules for
        // deprecated symbols only match imports from the deprecated sub-path,
        // avoiding false positives when the same component name exists in both
        // the main and deprecated paths (e.g., Dropdown, Select).
        let from_pkg = resolve_npm_package(&file_changes.file.to_string_lossy(), pkg_cache);

        for api_change in &file_changes.breaking_api_changes {
            let new_rules = api_change_to_rules(
                api_change,
                file_changes,
                file_pattern,
                from_pkg.as_deref(),
                &mut id_counts,
            );
            rules.extend(new_rules);
        }

        // Skip behavioral changes from test/demo/integration/example source
        // files.  These are test harnesses that happen to have common component
        // names (e.g., App, LoginPageDemo) and produce false positives when
        // matched against consumer code.
        let file_path_str = file_changes.file.to_string_lossy();
        let is_test_demo_file = file_path_str.contains("/demo")
            || file_path_str.contains("/test")
            || file_path_str.contains("/testdata/")
            || file_path_str.contains("/integration/")
            || file_path_str.contains("/examples/")
            || file_path_str.contains("/stories/");

        if !is_test_demo_file {
            for behavioral in &file_changes.breaking_behavioral_changes {
                let rule = behavioral_change_to_rule(
                    behavioral,
                    file_changes,
                    file_pattern,
                    from_pkg.as_deref(),
                    &mut id_counts,
                );
                rules.push(rule);
            }
        }
    }

    // ── P0-C + P1-A: Synthesize component-level IMPORT and review rules ──
    //
    // Aggregate API changes by parent component interface.  When an interface
    // has had most of its properties removed (gutted), emit an IMPORT rule for
    // the component itself (stripping the "Props" suffix).  When a component
    // has >= 3 distinct breaking API changes but NO prop-level rules already
    // covering it, emit a JSX_COMPONENT "review" rule.
    //
    // SUPPRESSION: When a component already has >= 2 prop-level rules (from
    // the per-change loop above), skip both P0-C and P1-A.  The specific
    // prop-level rules already fire at the exact JSX_PROP locations, so the
    // broad component-level rules would only add noise at already-covered
    // locations.  The prop-level rules tell you WHICH props broke; the
    // component-level rules only say "something broke."
    {
        struct ComponentInfo {
            total_changes: usize,
            removal_count: usize,
            from_pkg: Option<String>,
        }
        let mut component_map: BTreeMap<String, ComponentInfo> = BTreeMap::new();
        for file_changes in &report.changes {
            let from_pkg = resolve_npm_package(&file_changes.file.to_string_lossy(), pkg_cache);

            for api_change in &file_changes.breaking_api_changes {
                if !api_change.symbol.contains('.') {
                    continue;
                }
                let parts: Vec<&str> = api_change.symbol.splitn(2, '.').collect();
                let interface_name = parts[0].to_string();
                let entry = component_map
                    .entry(interface_name)
                    .or_insert(ComponentInfo {
                        total_changes: 0,
                        removal_count: 0,
                        from_pkg: from_pkg.clone(),
                    });
                entry.total_changes += 1;
                if api_change.change == ApiChangeType::Removed {
                    entry.removal_count += 1;
                }
            }
        }

        for (interface_name, info) in &component_map {
            let component_name = interface_name
                .strip_suffix("Props")
                .unwrap_or(interface_name);

            // P0-C: Interface with significant removals → IMPORT rule for the
            // component so consumers importing it are warned.
            // NOT suppressed by prop-level rules — prop rules use JSX_PROP
            // which fires at usage sites, not at the import line.  The import
            // is where the consumer needs to act (remove or replace the import).
            let mostly_removed = info.removal_count >= 1
                && (info.removal_count * 2 >= info.total_changes)
                && info.total_changes >= 2;
            if mostly_removed {
                let base_id = format!(
                    "semver-{}-component-import-deprecated",
                    sanitize_id(component_name)
                );
                let rule_id = unique_id(base_id, &mut id_counts);
                rules.push(KonveyorRule {
                    rule_id,
                    labels: vec![
                        "source=semver-analyzer".to_string(),
                        "change-type=component-removal".to_string(),
                        format!("kind=interface"),
                        "has-codemod=false".to_string(),
                    ],
                    effort: 3,
                    category: "mandatory".to_string(),
                    description: format!(
                        "{} has significant breaking changes — {} of {} props removed",
                        component_name, info.removal_count, info.total_changes
                    ),
                    message: format!(
                        "Interface '{}' had {} of {} properties removed, indicating \
                         the component was removed or merged into another component. \
                         Review your imports and migrate to the replacement.\n\
                         File: synthetic/{}.d.ts",
                        interface_name, info.removal_count, info.total_changes, component_name
                    ),
                    links: Vec::new(),
                    when: KonveyorCondition::FrontendReferenced {
                        referenced: FrontendReferencedFields {
                            pattern: format!("^{}$", regex_escape(component_name)),
                            location: "IMPORT".to_string(),
                            component: None,
                            parent: None,
                            value: None,
                            from: info.from_pkg.clone(),
                        },
                    },
                });
            }

            // P1-A (component-review) is intentionally omitted here.  The
            // prop-level rules from the per-change loop above already fire at
            // the exact JSX_PROP locations telling the consumer WHICH props
            // broke.  A broad "review all usages" JSX_COMPONENT rule would
            // only add noise at the same locations without new information.
        }
    }

    // Manifest changes
    for manifest in &report.manifest_changes {
        let rule = manifest_change_to_rule(manifest, file_pattern, &mut id_counts);
        rules.push(rule);
    }

    // Emit consumer CSS scanning rules when CSS version prefix changes are detected.
    // Extract the actual old prefix from the report data — no hardcoded library names.
    let css_prefix_changes = detect_css_prefix_changes(report);
    for (old_class_prefix, old_var_prefix) in &css_prefix_changes {
        // Consumer CSS/SCSS — stale CSS class prefix
        rules.push(KonveyorRule {
            rule_id: format!(
                "semver-consumer-css-stale-{}",
                sanitize_id(old_class_prefix)
            ),
            labels: vec![
                "source=semver-analyzer".to_string(),
                "change-type=css-class".to_string(),
            ],
            effort: 3,
            category: "mandatory".to_string(),
            description: format!(
                "Consumer CSS contains stale '{}' class prefix",
                old_class_prefix
            ),
            message: format!(
                "CSS/SCSS files reference '{}' class names which have been renamed. \
                 Update class references to the new prefix.",
                old_class_prefix
            ),
            links: Vec::new(),
            when: KonveyorCondition::FrontendCssClass {
                cssclass: FrontendPatternFields {
                    pattern: old_class_prefix.clone(),
                },
            },
        });

        // Consumer CSS/SCSS — stale CSS variable prefix
        rules.push(KonveyorRule {
            rule_id: format!(
                "semver-consumer-css-stale-var-{}",
                sanitize_id(old_var_prefix)
            ),
            labels: vec![
                "source=semver-analyzer".to_string(),
                "change-type=css-variable".to_string(),
            ],
            effort: 5,
            category: "mandatory".to_string(),
            description: format!(
                "Consumer CSS contains stale '{}' CSS variable prefix",
                old_var_prefix
            ),
            message: format!(
                "CSS/SCSS files reference '{}' CSS variables which have been renamed. \
                 Update variable references to the new prefix.",
                old_var_prefix
            ),
            links: Vec::new(),
            when: KonveyorCondition::FrontendCssVar {
                cssvar: FrontendPatternFields {
                    pattern: old_var_prefix.clone(),
                },
            },
        });
    }

    // ── P2-A: Composition rules (parent/child nesting) ──────────────────
    for entry in &rename_patterns.composition_rules {
        let base_id = format!(
            "semver-composition-{}-in-{}",
            sanitize_id(&entry.child_pattern),
            sanitize_id(&entry.parent),
        );
        let rule_id = unique_id(base_id, &mut id_counts);
        rules.push(KonveyorRule {
            rule_id,
            labels: vec![
                "source=semver-analyzer".to_string(),
                "change-type=composition".to_string(),
                "has-codemod=true".to_string(),
            ],
            effort: entry.effort,
            category: entry.category.clone(),
            description: entry.description.clone(),
            message: entry.description.clone(),
            links: Vec::new(),
            when: KonveyorCondition::FrontendReferenced {
                referenced: FrontendReferencedFields {
                    pattern: entry.child_pattern.clone(),
                    location: "JSX_COMPONENT".to_string(),
                    component: None,
                    parent: Some(entry.parent.clone()),
                    value: None,
                    from: entry.package.clone(),
                },
            },
        });
    }

    // ── P3-A: Prop renames ──────────────────────────────────────────────
    for entry in &rename_patterns.prop_renames {
        let desc = entry.description.clone().unwrap_or_else(|| {
            format!(
                "'{}' prop renamed to '{}' — update all usages",
                entry.old_prop, entry.new_prop
            )
        });
        let base_id = format!(
            "semver-prop-rename-{}-to-{}",
            sanitize_id(&entry.old_prop),
            sanitize_id(&entry.new_prop),
        );
        let rule_id = unique_id(base_id, &mut id_counts);
        rules.push(KonveyorRule {
            rule_id,
            labels: vec![
                "source=semver-analyzer".to_string(),
                "change-type=prop-rename".to_string(),
                "has-codemod=true".to_string(),
            ],
            effort: 1,
            category: "mandatory".to_string(),
            description: desc.clone(),
            message: desc,
            links: Vec::new(),
            when: KonveyorCondition::FrontendReferenced {
                referenced: FrontendReferencedFields {
                    pattern: format!("^{}$", regex_escape(&entry.old_prop)),
                    location: "JSX_PROP".to_string(),
                    component: Some(entry.components.clone()),
                    parent: None,
                    value: None,
                    from: entry.package.clone(),
                },
            },
        });
    }

    // ── P4-C: Value review rules ────────────────────────────────────────
    for entry in &rename_patterns.value_reviews {
        let base_id = format!(
            "semver-value-review-{}-{}-{}",
            sanitize_id(&entry.component),
            sanitize_id(&entry.prop),
            sanitize_id(&entry.value),
        );
        let rule_id = unique_id(base_id, &mut id_counts);
        rules.push(KonveyorRule {
            rule_id,
            labels: vec![
                "source=semver-analyzer".to_string(),
                "change-type=prop-value-review".to_string(),
                "has-codemod=true".to_string(),
            ],
            effort: entry.effort,
            category: entry.category.clone(),
            description: entry.description.clone(),
            message: entry.description.clone(),
            links: Vec::new(),
            when: KonveyorCondition::FrontendReferenced {
                referenced: FrontendReferencedFields {
                    pattern: format!("^{}$", regex_escape(&entry.prop)),
                    location: "JSX_PROP".to_string(),
                    component: Some(entry.component.clone()),
                    parent: None,
                    value: Some(entry.value.clone()),
                    from: entry.package.clone(),
                },
            },
        });
    }

    // ── Component warnings (DOM/CSS rendering changes without API change) ─
    for entry in &rename_patterns.component_warnings {
        let base_id = format!("semver-component-warning-{}", sanitize_id(&entry.pattern),);
        let rule_id = unique_id(base_id, &mut id_counts);
        rules.push(KonveyorRule {
            rule_id,
            labels: vec![
                "source=semver-analyzer".to_string(),
                "change-type=component-warning".to_string(),
                "impact=frontend-testing".to_string(),
                "has-codemod=false".to_string(),
            ],
            effort: entry.effort,
            category: entry.category.clone(),
            description: entry.description.clone(),
            message: entry.description.clone(),
            links: Vec::new(),
            when: KonveyorCondition::FrontendReferenced {
                referenced: FrontendReferencedFields {
                    pattern: entry.pattern.clone(),
                    location: "JSX_COMPONENT".to_string(),
                    component: None,
                    parent: None,
                    value: None,
                    from: entry.package.clone(),
                },
            },
        });
    }

    // ── P5: Missing import rules (and/not combinators) ──────────────────
    for entry in &rename_patterns.missing_imports {
        let base_id = format!(
            "semver-missing-import-{}",
            sanitize_id(&entry.missing_pattern),
        );
        let rule_id = unique_id(base_id, &mut id_counts);
        rules.push(KonveyorRule {
            rule_id,
            labels: vec![
                "source=semver-analyzer".to_string(),
                "change-type=missing-import".to_string(),
                "has-codemod=false".to_string(),
            ],
            effort: entry.effort,
            category: entry.category.clone(),
            description: entry.description.clone(),
            message: entry.description.clone(),
            links: Vec::new(),
            when: KonveyorCondition::And {
                and: vec![
                    KonveyorCondition::FileContent {
                        filecontent: FileContentFields {
                            pattern: entry.has_pattern.clone(),
                            file_pattern: entry.file_pattern.clone(),
                        },
                    },
                    KonveyorCondition::FileContentNegated {
                        negated: true,
                        filecontent: FileContentFields {
                            pattern: entry.missing_pattern.clone(),
                            file_pattern: entry.file_pattern.clone(),
                        },
                    },
                ],
            },
        });
    }

    rules
}

/// Analyze the report to find token object member keys and member renames.
///
/// For each `type_changed` entry whose before/after contains `["member_key"]`
/// patterns (token objects), extracts the member key sets and diffs them.
///
/// Returns:
/// - `covered_symbols`: symbols that appear as member keys in a parent token's
///   type_changed entry. Individual `Removed` rules for these are redundant.
/// - `member_renames`: old_member → new_member mappings derived from diffing
///   member key sets using the supplied rename patterns.
pub fn analyze_token_members(
    report: &AnalysisReport,
    rename_patterns: &RenamePatterns,
) -> (BTreeSet<String>, HashMap<String, String>) {
    static MEMBER_KEY_RE: std::sync::LazyLock<regex::Regex> = std::sync::LazyLock::new(|| {
        regex::Regex::new(r#"\["([a-zA-Z_][a-zA-Z_0-9]*)"\]"#).unwrap()
    });

    let mut covered_symbols: BTreeSet<String> = BTreeSet::new();
    let mut member_renames: HashMap<String, String> = HashMap::new();

    for file_changes in &report.changes {
        for api_change in &file_changes.breaking_api_changes {
            if api_change.change != ApiChangeType::TypeChanged {
                continue;
            }

            let before = match &api_change.before {
                Some(b) if b.contains("[\"") => b,
                _ => continue,
            };
            let after = match &api_change.after {
                Some(a) if a.contains("[\"") => a,
                _ => continue,
            };

            // Extract member keys from before and after
            let old_keys: BTreeSet<String> = MEMBER_KEY_RE
                .captures_iter(before)
                .map(|c| c[1].to_string())
                .filter(|k| k != "name" && k != "value" && k != "values" && k != "var")
                .collect();

            let new_keys: BTreeSet<String> = MEMBER_KEY_RE
                .captures_iter(after)
                .map(|c| c[1].to_string())
                .filter(|k| k != "name" && k != "value" && k != "values" && k != "var")
                .collect();

            if old_keys.len() < 3 || new_keys.len() < 3 {
                continue; // Not a token object — too few members
            }

            // All old member keys are "covered" — individual removal rules are redundant
            for key in &old_keys {
                covered_symbols.insert(key.clone());
            }

            // Diff member keys to find renames
            let removed: BTreeSet<&String> = old_keys.difference(&new_keys).collect();
            let added: BTreeSet<&String> = new_keys.difference(&old_keys).collect();

            // Try to match removed→added using rename patterns
            for old_key in &removed {
                if let Some(expected_new) = rename_patterns.find_replacement(old_key) {
                    if added.contains(&expected_new) {
                        member_renames.insert(old_key.to_string(), expected_new);
                    }
                }
            }
        }
    }

    (covered_symbols, member_renames)
}

/// Suppress redundant individual token removal rules.
///
/// When a parent token object has a `type_changed` entry, individual
/// `Removed` rules for its member keys are redundant noise. This function
/// filters them out and returns the remaining rules.
pub fn suppress_redundant_token_rules(
    rules: Vec<KonveyorRule>,
    covered_symbols: &BTreeSet<String>,
) -> Vec<KonveyorRule> {
    if covered_symbols.is_empty() {
        return rules;
    }

    let before_count = rules.len();
    let rules: Vec<KonveyorRule> = rules
        .into_iter()
        .filter(|rule| {
            // Only suppress rules that are:
            // 1. Removal rules (change-type=removed)
            // 2. For constants (kind=constant)
            // 3. Whose symbol name is in the covered set
            let is_removal = rule.labels.iter().any(|l| l == "change-type=removed");
            let is_constant = rule.labels.iter().any(|l| l == "kind=constant");

            if !is_removal || !is_constant {
                return true; // Keep non-removal, non-constant rules
            }

            // Only suppress per-file individual token .d.ts rules, not index-level re-exports.
            // Index-level rules are what consumers actually import.
            let is_index = rule.message.lines().any(|l| l.contains("index.d.ts"));
            if is_index {
                return true; // Keep index-level rules
            }

            // Extract the symbol name from the description
            // Description format: "Exported constant `symbol_name` was removed"
            let symbol = rule.description.split('`').nth(1).unwrap_or("");

            !covered_symbols.contains(symbol)
        })
        .collect();

    let suppressed = before_count - rules.len();
    if suppressed > 0 {
        eprintln!(
            "Suppressed {} redundant token removal rules (covered by parent type_changed)",
            suppressed
        );
    }

    rules
}

/// Consolidate rules by grouping related changes into composite rules.
/// Consolidate rules by grouping related rules into single combined rules.
///
/// Returns the consolidated rules AND a mapping from old rule IDs to the new
/// consolidated rule ID.  The mapping is used to re-key fix strategies so they
/// match the post-consolidation rule IDs that appear in kantra output.
pub fn consolidate_rules(rules: Vec<KonveyorRule>) -> (Vec<KonveyorRule>, HashMap<String, String>) {
    let mut groups: BTreeMap<String, Vec<KonveyorRule>> = BTreeMap::new();
    for rule in rules {
        let key = consolidation_key(&rule);
        groups.entry(key).or_default().push(rule);
    }
    let mut consolidated = Vec::new();
    let mut id_mapping = HashMap::new();
    for (_key, group) in groups {
        if group.len() == 1 {
            let rule = group.into_iter().next().unwrap();
            id_mapping.insert(rule.rule_id.clone(), rule.rule_id.clone());
            consolidated.push(rule);
        } else {
            let old_ids: Vec<String> = group.iter().map(|r| r.rule_id.clone()).collect();
            let merged = merge_rule_group(group);
            let new_id = merged.rule_id.clone();
            for old_id in old_ids {
                id_mapping.insert(old_id, new_id.clone());
            }
            consolidated.push(merged);
        }
    }
    (consolidated, id_mapping)
}

fn consolidation_key(rule: &KonveyorRule) -> String {
    let change_type = rule
        .labels
        .iter()
        .find(|l| l.starts_with("change-type="))
        .map(|l| l.strip_prefix("change-type=").unwrap_or("unknown"))
        .unwrap_or("unknown");
    let kind = rule
        .labels
        .iter()
        .find(|l| l.starts_with("kind="))
        .map(|l| l.strip_prefix("kind=").unwrap_or(""))
        .unwrap_or("");
    let file_key = rule
        .message
        .lines()
        .find(|l| l.starts_with("File:"))
        .map(|l| l.trim_start_matches("File:").trim())
        .unwrap_or("unknown");

    if change_type == "manifest" {
        let field = rule
            .labels
            .iter()
            .find(|l| l.starts_with("manifest-field="))
            .map(|l| l.strip_prefix("manifest-field=").unwrap_or(""))
            .unwrap_or("");
        return format!("manifest-{}-{}", field, change_type);
    }

    // Removed constants: consolidate by package, not by file.
    // This collapses hundreds of individual token removals into 1-2 rules per package.
    // BUT: only do this for token-style constants (lowercase/underscore names like
    // `c_button_hover_Color`). PascalCase component constants (`DropdownItem`, `Select`)
    // should NOT be merged into token regex groups — they need individual IMPORT rules.
    if change_type == "removed" && kind == "constant" {
        let symbol = rule.description.split('`').nth(1).unwrap_or("");
        let is_component_constant = symbol
            .chars()
            .next()
            .map_or(false, |c| c.is_ascii_uppercase());
        if !is_component_constant {
            let package = extract_package_from_path(file_key);
            return format!("{}-constant-removed", package);
        }
    }

    format!("{}-{}-{}", file_key, kind, change_type)
}

/// Build a package name cache from the report's file paths.
///
/// For each unique `packages/<dir>/` prefix, reads `<dir>/package.json` to get
/// the npm `name` field. Falls back to the directory name if `package.json`
/// is not readable (e.g., when working from a report without the repo present).
///
/// Also handles `/deprecated/` and `/next/` subpaths by appending them.
pub fn build_package_name_cache(report: &AnalysisReport) -> HashMap<String, String> {
    let mut cache: HashMap<String, String> = HashMap::new();
    let repo_path = &report.repository;

    for file_changes in &report.changes {
        let file_str = file_changes.file.to_string_lossy();
        let parts: Vec<&str> = file_str.split('/').collect();

        if let Some(pkg_idx) = parts.iter().position(|&p| p == "packages") {
            if let Some(pkg_dir_name) = parts.get(pkg_idx + 1) {
                if cache.contains_key(*pkg_dir_name) {
                    continue;
                }

                // Try to read package.json from the repo
                let pkg_dir = repo_path.join("packages").join(pkg_dir_name);
                let pkg_json_path = pkg_dir.join("package.json");

                let npm_name = if let Ok(content) = std::fs::read_to_string(&pkg_json_path) {
                    serde_json::from_str::<serde_json::Value>(&content)
                        .ok()
                        .and_then(|v| v.get("name")?.as_str().map(|s| s.to_string()))
                } else {
                    None
                };

                let name = npm_name.unwrap_or_else(|| pkg_dir_name.to_string());
                cache.insert(pkg_dir_name.to_string(), name);
            }
        }
    }

    if !cache.is_empty() {
        eprintln!("Package name cache: {:?}", cache);
    }

    cache
}

/// Look up the npm package name for a file path using the cache.
///
/// Returns the package name with `/deprecated` or `/next` suffix when the
/// source file lives under those directories.  Sub-path packages are
/// regex-anchored with `^...$` so that the provider's unanchored
/// `Regex::is_match` on the `from` field matches only the exact sub-path
/// import and not the base package.  For example:
///
/// - `@patternfly/react-core` (unanchored) matches imports from both
///   `@patternfly/react-core` and `@patternfly/react-core/deprecated`.
/// - `^@patternfly/react-core/deprecated$` (anchored) matches ONLY imports
///   from `@patternfly/react-core/deprecated`.
///
/// This prevents false positives when a component name exists in both the
/// deprecated and non-deprecated paths (e.g., `Dropdown`, `Select`).
fn resolve_npm_package(file_path: &str, cache: &HashMap<String, String>) -> Option<String> {
    let parts: Vec<&str> = file_path.split('/').collect();
    let pkg_idx = parts.iter().position(|&p| p == "packages")?;
    let pkg_dir_name = parts.get(pkg_idx + 1)?;

    let base_name = cache.get(*pkg_dir_name)?;

    let has_deprecated = parts.iter().any(|&p| p == "deprecated");
    let has_next = parts.iter().any(|&p| p == "next");

    if has_deprecated {
        // Anchor so it won't match the base package
        Some(format!("^{}/deprecated$", regex_escape(base_name)))
    } else if has_next {
        Some(format!("^{}/next$", regex_escape(base_name)))
    } else {
        Some(base_name.clone())
    }
}

/// Extract a package name from a file path for consolidation grouping.
///
/// `packages/react-tokens/dist/esm/index.d.ts` → `react-tokens`
/// `packages/react-core/dist/esm/components/Button/Button.d.ts` → `react-core`
/// `packages/react-core/dist/esm/deprecated/components/...` → `react-core-deprecated`
fn extract_package_from_path(path: &str) -> String {
    let parts: Vec<&str> = path.split('/').collect();
    if let Some(pkg_idx) = parts.iter().position(|&p| p == "packages") {
        if let Some(pkg_name) = parts.get(pkg_idx + 1) {
            let has_deprecated = parts.iter().any(|&p| p == "deprecated");
            if has_deprecated {
                return format!("{}-deprecated", pkg_name);
            }
            return pkg_name.to_string();
        }
    }
    // Fallback: use the first meaningful directory
    path.split('/')
        .find(|s| !s.is_empty())
        .unwrap_or("unknown")
        .to_string()
}

fn merge_rule_group(group: Vec<KonveyorRule>) -> KonveyorRule {
    let count = group.len();
    let first_rule_id = group[0].rule_id.clone();
    let first_category = group[0].category.clone();
    let effort = group.iter().map(|r| r.effort).max().unwrap_or(1);
    let mut all_labels: BTreeSet<String> = BTreeSet::new();
    for rule in &group {
        for label in &rule.labels {
            all_labels.insert(label.clone());
        }
    }
    let labels: Vec<String> = all_labels.into_iter().collect();
    let descriptions: Vec<&str> = group.iter().map(|r| r.description.as_str()).collect();
    let unique_descriptions: Vec<&str> = {
        let mut seen = BTreeSet::new();
        descriptions
            .iter()
            .filter(|d| seen.insert(**d))
            .copied()
            .collect()
    };
    let message = if unique_descriptions.len() <= 5 {
        unique_descriptions
            .iter()
            .map(|d| format!("- {}", d))
            .collect::<Vec<_>>()
            .join("\n")
    } else {
        let shown: Vec<String> = unique_descriptions[..4]
            .iter()
            .map(|d| format!("- {}", d))
            .collect();
        format!(
            "{}\n- ... and {} more changes",
            shown.join("\n"),
            unique_descriptions.len() - 4
        )
    };
    let description = format!("{} related changes", count);
    let rule_id = format!("{}-group-{}", first_rule_id, count);

    // For large groups of removed constants, generate a single broad pattern
    // instead of an or: with hundreds of individual conditions.
    // Extract the common symbol prefix and build one regex from it.
    let is_large_removed_constant = count > 20
        && labels.iter().any(|l| l == "change-type=removed")
        && labels.iter().any(|l| l == "kind=constant");

    let when = if is_large_removed_constant {
        // Extract symbol names from descriptions to find common prefix
        let symbols: Vec<&str> = descriptions
            .iter()
            .filter_map(|d| d.split('`').nth(1))
            .collect();
        let pattern = build_common_prefix_pattern(&symbols);

        // Try to extract the package name from the labels (package=@scope/name)
        let from_pkg: Option<String> = labels
            .iter()
            .find(|l| l.starts_with("package="))
            .map(|l| l.strip_prefix("package=").unwrap_or("").to_string());

        // If we have a package scope, use frontend.referenced with IMPORT location
        // This matches exactly how the hand-crafted rules work:
        //   frontend.referenced: { pattern: "^c_[a-z]", location: IMPORT, from: "@patternfly/react-tokens" }
        if from_pkg.is_some() {
            KonveyorCondition::FrontendReferenced {
                referenced: FrontendReferencedFields {
                    pattern,
                    location: "IMPORT".to_string(),
                    component: None,
                    parent: None,
                    value: None,
                    from: from_pkg,
                },
            }
        } else {
            let file_pattern = extract_file_pattern_from_condition(&group[0].when)
                .unwrap_or_else(|| "*.{ts,tsx,js,jsx,mjs,cjs}".to_string());
            KonveyorCondition::FileContent {
                filecontent: FileContentFields {
                    pattern,
                    file_pattern,
                },
            }
        }
    } else {
        let conditions: Vec<KonveyorCondition> = group.into_iter().map(|r| r.when).collect();
        if conditions.len() == 1 {
            conditions.into_iter().next().unwrap()
        } else {
            let unique = dedup_conditions(conditions);
            if unique.len() == 1 {
                unique.into_iter().next().unwrap()
            } else {
                KonveyorCondition::Or { or: unique }
            }
        }
    };

    KonveyorRule {
        rule_id,
        labels,
        effort,
        category: first_category,
        description,
        message,
        links: Vec::new(),
        when,
    }
}

/// Build a regex pattern from the common prefix of a list of symbol names.
///
/// Given `["c_button_hover_Color", "c_button_focus_Color", "c_accordion_active_Color"]`,
/// finds the common prefixes and builds a pattern like `^(c_button_|c_accordion_)`.
///
/// For very large groups with no common prefix, falls back to matching
/// any symbol that looks like a component token: `^(c_|global_|chart_)`.
fn build_common_prefix_pattern(symbols: &[&str]) -> String {
    if symbols.is_empty() {
        return ".*".to_string();
    }

    // Group symbols by their first two segments (e.g., "c_button" from "c_button_hover_Color")
    let mut prefix_groups: BTreeMap<String, usize> = BTreeMap::new();
    for sym in symbols {
        // Take up to the second underscore for grouping
        let parts: Vec<&str> = sym.splitn(3, '_').collect();
        let prefix = if parts.len() >= 2 {
            format!("{}_{}", parts[0], parts[1])
        } else {
            sym.to_string()
        };
        *prefix_groups.entry(prefix).or_insert(0) += 1;
    }

    // Build alternation from top-level prefixes
    let top_prefixes: Vec<&str> = symbols
        .iter()
        .filter_map(|s| s.split('_').next())
        .collect::<BTreeSet<&str>>()
        .into_iter()
        .collect();

    if top_prefixes.len() <= 5 {
        // Few top-level prefixes — use them directly
        let alts: Vec<String> = top_prefixes.iter().map(|p| format!("{}_", p)).collect();
        format!(r"\b({})", alts.join("|"))
    } else {
        // Many prefixes — just match any word-boundary token-like identifier
        r"\b[a-z][a-z0-9_]+_(Color|BackgroundColor|FontSize|BorderWidth|BoxShadow|FontWeight|Width|Height|ZIndex)\b".to_string()
    }
}

/// Extract the file pattern from an existing condition (for reuse in consolidated rules).
fn extract_file_pattern_from_condition(condition: &KonveyorCondition) -> Option<String> {
    match condition {
        KonveyorCondition::FileContent { filecontent } => Some(filecontent.file_pattern.clone()),
        KonveyorCondition::Or { or } => or.first().and_then(extract_file_pattern_from_condition),
        _ => None,
    }
}

/// Detect CSS version prefix changes from the report data.
///
/// Scans `type_changed` entries for CSS custom property prefix transformations
/// (e.g., `--pf-v5-` → `--pf-v6-`). Returns the old prefixes as
/// `(class_prefix, var_prefix)` pairs derived from the data.
///
/// The class prefix is the var prefix without the leading `--`
/// (e.g., `--pf-v5-` → `pf-v5-`).
fn detect_css_prefix_changes(report: &AnalysisReport) -> Vec<(String, String)> {
    let mut seen = BTreeSet::new();
    let mut results = Vec::new();

    for file_changes in &report.changes {
        for api_change in &file_changes.breaking_api_changes {
            if api_change.change != ApiChangeType::TypeChanged {
                continue;
            }
            if let Some((old_prefix, _new_prefix)) = detect_version_prefix(&api_change.description)
            {
                if seen.insert(old_prefix.clone()) {
                    // Derive the class prefix from the var prefix
                    // --pf-v5- → pf-v5-
                    let class_prefix = old_prefix.trim_start_matches('-').to_string();
                    results.push((class_prefix, old_prefix));
                }
            }
        }
    }

    results
}

fn dedup_conditions(conditions: Vec<KonveyorCondition>) -> Vec<KonveyorCondition> {
    let mut seen = BTreeSet::new();
    let mut unique = Vec::new();
    for cond in conditions {
        let key = serde_json::to_string(&cond).unwrap_or_default();
        if seen.insert(key) {
            unique.push(cond);
        }
    }
    unique
}

/// Generate fix strategy mappings from an AnalysisReport.
pub fn generate_fix_strategies(
    report: &AnalysisReport,
    rules: &[KonveyorRule],
    rename_patterns: &RenamePatterns,
    member_renames: &HashMap<String, String>,
) -> HashMap<String, FixStrategyEntry> {
    let mut strategies = HashMap::new();
    let mut rule_idx = 0;
    for file_changes in &report.changes {
        let file_path = file_changes.file.to_string_lossy();
        for api_change in &file_changes.breaking_api_changes {
            if rule_idx < rules.len() {
                if let Some(entry) =
                    api_change_to_strategy(api_change, rename_patterns, member_renames, &file_path)
                {
                    strategies.insert(rules[rule_idx].rule_id.clone(), entry);
                }
                rule_idx += 1;
            }
        }
        for _behavioral in &file_changes.breaking_behavioral_changes {
            if rule_idx < rules.len() {
                strategies.insert(
                    rules[rule_idx].rule_id.clone(),
                    FixStrategyEntry {
                        strategy: "LlmAssisted".into(),
                        from: None,
                        to: None,
                        component: None,
                        prop: None,
                    },
                );
            }
            rule_idx += 1;
        }
    }
    for _manifest in &report.manifest_changes {
        if rule_idx < rules.len() {
            strategies.insert(
                rules[rule_idx].rule_id.clone(),
                FixStrategyEntry {
                    strategy: "Manual".into(),
                    from: None,
                    to: None,
                    component: None,
                    prop: None,
                },
            );
        }
        rule_idx += 1;
    }

    // Also add strategies for synthetic rules (composition, prop renames,
    // value reviews, component warnings, missing imports) that are generated
    // after the report iteration.  These rules start after rule_idx.
    for rule in rules.iter().skip(rule_idx) {
        let change_type = rule
            .labels
            .iter()
            .find(|l| l.starts_with("change-type="))
            .map(|l| l.strip_prefix("change-type=").unwrap_or(""))
            .unwrap_or("");
        let entry = match change_type {
            "composition" | "prop-rename" => {
                // Extract from/to from the rule description if possible
                let from = rule.description.split('\'').nth(1).map(String::from);
                let to = rule.description.split('\'').nth(3).map(String::from);
                FixStrategyEntry {
                    strategy: "Rename".into(),
                    from,
                    to,
                    component: None,
                    prop: None,
                }
            }
            "prop-value-review" | "component-warning" | "missing-import" => FixStrategyEntry {
                strategy: "Manual".into(),
                from: None,
                to: None,
                component: None,
                prop: None,
            },
            "component-removal" => FixStrategyEntry {
                strategy: "LlmAssisted".into(),
                from: None,
                to: None,
                component: None,
                prop: None,
            },
            _ => FixStrategyEntry {
                strategy: "Manual".into(),
                from: None,
                to: None,
                component: None,
                prop: None,
            },
        };
        strategies.insert(rule.rule_id.clone(), entry);
    }

    strategies
}

/// Write fix strategies JSON to the fix-guidance directory.
pub fn write_fix_strategies(
    fix_dir: &Path,
    strategies: &HashMap<String, FixStrategyEntry>,
) -> Result<()> {
    let path = fix_dir.join("fix-strategies.json");
    let json =
        serde_json::to_string_pretty(strategies).context("Failed to serialize fix strategies")?;
    std::fs::write(&path, &json).with_context(|| format!("Failed to write {}", path.display()))?;
    Ok(())
}

/// A machine-readable fix strategy entry.
#[derive(Debug, Serialize)]
pub struct FixStrategyEntry {
    pub strategy: String,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub from: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub to: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub component: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub prop: Option<String>,
}

fn api_change_to_strategy(
    change: &ApiChange,
    rename_patterns: &RenamePatterns,
    member_renames: &HashMap<String, String>,
    file_path: &str,
) -> Option<FixStrategyEntry> {
    match change.change {
        ApiChangeType::Renamed => {
            let before = change.before.as_deref().unwrap_or("");
            let after = change.after.as_deref().unwrap_or("");
            if after.contains("/deprecated/") && !before.contains("/deprecated/") {
                return Some(FixStrategyEntry {
                    strategy: "ImportPathChange".into(),
                    from: extract_package_path(before),
                    to: extract_package_path(after),
                    component: None,
                    prop: None,
                });
            }
            if before == after || extract_leaf_symbol(before) == extract_leaf_symbol(after) {
                let fp = extract_package_path(before);
                let tp = extract_package_path(after);
                if fp.is_some() && tp.is_some() && fp != tp {
                    return Some(FixStrategyEntry {
                        strategy: "ImportPathChange".into(),
                        from: fp,
                        to: tp,
                        component: None,
                        prop: None,
                    });
                }
            }
            Some(FixStrategyEntry {
                strategy: "Rename".into(),
                from: Some(extract_leaf_symbol(before).into()),
                to: Some(extract_leaf_symbol(after).into()),
                component: None,
                prop: None,
            })
        }
        ApiChangeType::TypeChanged | ApiChangeType::SignatureChanged => {
            // Union member value change
            if let Some(ref before) = change.before {
                if (before.starts_with('\'') && before.ends_with('\''))
                    || (before.starts_with('"') && before.ends_with('"'))
                {
                    let value = &before[1..before.len() - 1];
                    let (component, prop) = if change.symbol.contains('.') {
                        let parts: Vec<&str> = change.symbol.splitn(2, '.').collect();
                        (Some(parts[0].to_string()), Some(parts[1].to_string()))
                    } else {
                        (None, None)
                    };
                    return Some(FixStrategyEntry {
                        strategy: "PropValueChange".into(),
                        from: Some(value.into()),
                        to: None,
                        component,
                        prop,
                    });
                }
            }
            // CSS variable prefix change
            if let Some((fp, tp)) = detect_version_prefix(&change.description) {
                return Some(FixStrategyEntry {
                    strategy: "CssVariablePrefix".into(),
                    from: Some(fp),
                    to: Some(tp),
                    component: None,
                    prop: None,
                });
            }
            let (component, prop) = if change.symbol.contains('.') {
                let parts: Vec<&str> = change.symbol.splitn(2, '.').collect();
                (Some(parts[0].to_string()), Some(parts[1].to_string()))
            } else {
                (None, None)
            };
            Some(FixStrategyEntry {
                strategy: "PropTypeChange".into(),
                from: change.before.clone(),
                to: change.after.clone(),
                component,
                prop,
            })
        }
        ApiChangeType::Removed => {
            if matches!(change.kind, ApiChangeKind::Property | ApiChangeKind::Field) {
                let (component, prop) = if change.symbol.contains('.') {
                    let parts: Vec<&str> = change.symbol.splitn(2, '.').collect();
                    (Some(parts[0].into()), Some(parts[1].into()))
                } else {
                    (None, Some(change.symbol.clone()))
                };
                Some(FixStrategyEntry {
                    strategy: "RemoveProp".into(),
                    from: None,
                    to: None,
                    component,
                    prop,
                })
            } else if let Some(new_name) = member_renames.get(&change.symbol) {
                // Member rename derived from parent token object diff
                Some(FixStrategyEntry {
                    strategy: "Rename".into(),
                    from: Some(change.symbol.clone()),
                    to: Some(new_name.clone()),
                    component: None,
                    prop: None,
                })
            } else if let Some(replacement) = rename_patterns.find_replacement(&change.symbol) {
                // User-supplied rename pattern matched
                Some(FixStrategyEntry {
                    strategy: "Rename".into(),
                    from: Some(change.symbol.clone()),
                    to: Some(replacement),
                    component: None,
                    prop: None,
                })
            } else if file_path.contains("/deprecated/") {
                // Symbol from a deprecated path that was fully removed.
                // The LLM can suggest the replacement component.
                Some(FixStrategyEntry {
                    strategy: "LlmAssisted".into(),
                    from: Some(change.symbol.clone()),
                    to: None,
                    component: None,
                    prop: None,
                })
            } else {
                Some(FixStrategyEntry {
                    strategy: "Manual".into(),
                    from: None,
                    to: None,
                    component: None,
                    prop: None,
                })
            }
        }
        ApiChangeType::VisibilityChanged => Some(FixStrategyEntry {
            strategy: "Manual".into(),
            from: None,
            to: None,
            component: None,
            prop: None,
        }),
    }
}

fn extract_package_path(qualified_name: &str) -> Option<String> {
    let parts: Vec<&str> = qualified_name.split('/').collect();
    let pkg_idx = parts.iter().position(|&p| p == "packages")?;
    let pkg_name = parts.get(pkg_idx + 1)?;
    let internal_parts: Vec<&str> = parts[parts.iter().position(|&p| p == "dist")?..].to_vec();
    let has_deprecated = internal_parts.iter().any(|&p| p == "deprecated");
    let has_next = internal_parts.iter().any(|&p| p == "next");
    let mut path = pkg_name.to_string();
    if has_deprecated {
        path.push_str("/deprecated");
    } else if has_next {
        path.push_str("/next");
    }
    Some(path)
}

fn detect_version_prefix(description: &str) -> Option<(String, String)> {
    static PREFIX_RE: std::sync::LazyLock<regex::Regex> =
        std::sync::LazyLock::new(|| regex::Regex::new(r"--([a-zA-Z]+-v)(\d+)-").unwrap());
    let mut prefixes: Vec<String> = Vec::new();
    for cap in PREFIX_RE.captures_iter(description) {
        let prefix = format!("--{}{}-", &cap[1], &cap[2]);
        if !prefixes.contains(&prefix) {
            prefixes.push(prefix);
        }
        if prefixes.len() == 2 {
            break;
        }
    }
    if prefixes.len() == 2 {
        let base1: String = prefixes[0]
            .chars()
            .take_while(|c| !c.is_ascii_digit())
            .collect();
        let base2: String = prefixes[1]
            .chars()
            .take_while(|c| !c.is_ascii_digit())
            .collect();
        if base1 == base2 {
            return Some((prefixes[0].clone(), prefixes[1].clone()));
        }
    }
    None
}

/// Generate fix guidance entries from an `AnalysisReport`.
///
/// Each rule gets a corresponding fix entry describing what to do about
/// the breaking change: strategy, confidence, concrete instructions, and
/// before/after examples where available.
pub fn generate_fix_guidance(
    report: &AnalysisReport,
    rules: &[KonveyorRule],
    file_pattern: &str,
) -> FixGuidanceDoc {
    let mut fixes = Vec::new();
    let mut rule_idx = 0;

    // API + behavioral changes (per-file, in same order as generate_rules)
    for file_changes in &report.changes {
        for api_change in &file_changes.breaking_api_changes {
            if rule_idx < rules.len() {
                let fix = api_change_to_fix(
                    api_change,
                    file_changes,
                    &rules[rule_idx].rule_id,
                    file_pattern,
                );
                fixes.push(fix);
                rule_idx += 1;
            }
        }
        for behavioral in &file_changes.breaking_behavioral_changes {
            if rule_idx < rules.len() {
                let fix =
                    behavioral_change_to_fix(behavioral, file_changes, &rules[rule_idx].rule_id);
                fixes.push(fix);
                rule_idx += 1;
            }
        }
    }

    // Manifest changes
    for manifest in &report.manifest_changes {
        if rule_idx < rules.len() {
            let fix = manifest_change_to_fix(manifest, &rules[rule_idx].rule_id);
            fixes.push(fix);
            rule_idx += 1;
        }
    }

    let auto_fixable = fixes
        .iter()
        .filter(|f| matches!(f.confidence, FixConfidence::Exact | FixConfidence::High))
        .count();
    let manual_only = fixes
        .iter()
        .filter(|f| matches!(f.source, FixSource::Manual))
        .count();
    let needs_review = fixes.len() - auto_fixable - manual_only;

    FixGuidanceDoc {
        migration: MigrationInfo {
            from_ref: report.comparison.from_ref.clone(),
            to_ref: report.comparison.to_ref.clone(),
            generated_by: format!("semver-analyzer v{}", report.metadata.tool_version),
        },
        summary: FixSummary {
            total_fixes: fixes.len(),
            auto_fixable,
            needs_review,
            manual_only,
        },
        fixes,
    }
}

/// Write a Konveyor ruleset directory.
///
/// Creates:
///   `<output_dir>/ruleset.yaml`         — ruleset metadata
///   `<output_dir>/breaking-changes.yaml` — all generated rules
pub fn write_ruleset_dir(
    output_dir: &Path,
    ruleset_name: &str,
    report: &AnalysisReport,
    rules: &[KonveyorRule],
) -> Result<()> {
    std::fs::create_dir_all(output_dir)
        .with_context(|| format!("Failed to create output directory {}", output_dir.display()))?;

    // Write ruleset.yaml
    let from_ref = &report.comparison.from_ref;
    let to_ref = &report.comparison.to_ref;
    let ruleset = KonveyorRuleset {
        name: ruleset_name.to_string(),
        description: format!(
            "Breaking changes detected between {} and {} by semver-analyzer v{}",
            from_ref, to_ref, report.metadata.tool_version
        ),
        labels: vec!["source=semver-analyzer".to_string()],
    };

    let ruleset_path = output_dir.join("ruleset.yaml");
    let ruleset_yaml = serde_yaml::to_string(&ruleset).context("Failed to serialize ruleset")?;
    std::fs::write(&ruleset_path, &ruleset_yaml)
        .with_context(|| format!("Failed to write {}", ruleset_path.display()))?;

    // Write rules file
    let rules_path = output_dir.join("breaking-changes.yaml");
    let rules_yaml = serde_yaml::to_string(&rules).context("Failed to serialize rules")?;
    std::fs::write(&rules_path, &rules_yaml)
        .with_context(|| format!("Failed to write {}", rules_path.display()))?;

    Ok(())
}

/// Write fix guidance to a separate sibling directory.
///
/// Given the ruleset `output_dir`, creates a `fix-guidance/` directory
/// next to it and writes `fix-guidance.yaml` there.
///
/// Example: if `output_dir` is `./rules`, writes to `./fix-guidance/fix-guidance.yaml`.
pub fn write_fix_guidance_dir(
    output_dir: &Path,
    fix_guidance: &FixGuidanceDoc,
) -> Result<std::path::PathBuf> {
    let fix_dir = fix_guidance_dir_for(output_dir);

    std::fs::create_dir_all(&fix_dir).with_context(|| {
        format!(
            "Failed to create fix guidance directory {}",
            fix_dir.display()
        )
    })?;

    let fix_path = fix_dir.join("fix-guidance.yaml");
    let fix_yaml =
        serde_yaml::to_string(fix_guidance).context("Failed to serialize fix guidance")?;
    std::fs::write(&fix_path, &fix_yaml)
        .with_context(|| format!("Failed to write {}", fix_path.display()))?;

    Ok(fix_dir)
}

/// Compute the fix-guidance sibling directory path for a given ruleset output dir.
///
/// `./my-rules` → `./fix-guidance`
/// `./output/rules` → `./output/fix-guidance`
pub fn fix_guidance_dir_for(output_dir: &Path) -> std::path::PathBuf {
    let parent = output_dir.parent().unwrap_or(Path::new("."));
    parent.join("fix-guidance")
}

// ── Rule generators ─────────────────────────────────────────────────────

fn api_change_to_rules(
    change: &ApiChange,
    file_changes: &FileChanges,
    file_pattern: &str,
    from_pkg: Option<&str>,
    id_counts: &mut HashMap<String, usize>,
) -> Vec<KonveyorRule> {
    let file_path = file_changes.file.display().to_string();
    let leaf_symbol = extract_leaf_symbol(&change.symbol);
    let effort = effort_for_api_change(&change.change);
    let change_type_label = api_change_type_label(&change.change);

    let base_id = format!(
        "semver-{}-{}-{}",
        sanitize_id(&file_path),
        sanitize_id(&change.symbol),
        change_type_label,
    );
    let rule_id = unique_id(base_id.clone(), id_counts);

    let message = build_api_message(change, &file_path);

    let mut labels = vec![
        "source=semver-analyzer".to_string(),
        format!("change-type={}", change_type_label),
        format!("kind={}", api_kind_label(&change.kind)),
    ];

    // Infer has-codemod from the change type
    let has_codemod = matches!(
        change.change,
        ApiChangeType::Renamed | ApiChangeType::SignatureChanged | ApiChangeType::TypeChanged
    );
    labels.push(format!("has-codemod={}", has_codemod));

    if let Some(pkg) = from_pkg {
        labels.push(format!("package={}", pkg));
    }

    let condition = build_frontend_condition(change, leaf_symbol, from_pkg);

    let mut rules = vec![KonveyorRule {
        rule_id,
        labels: labels.clone(),
        effort,
        category: "mandatory".to_string(),
        description: change.description.clone(),
        message,
        links: Vec::new(),
        when: condition,
    }];

    // P4-B: For type_changed Property/Field changes, check for removed union
    // member values and emit per-value rules so the `value` constraint fires.
    if matches!(change.kind, ApiChangeKind::Property | ApiChangeKind::Field)
        && change.change == ApiChangeType::TypeChanged
    {
        let removed_values = extract_removed_union_values(change);
        if removed_values.len() >= 2 {
            // Extract parent component for scoping
            let parent_component = if change.symbol.contains('.') {
                let parts: Vec<&str> = change.symbol.splitn(2, '.').collect();
                Some(format!("^{}$", regex_escape(parts[0])))
            } else {
                None
            };
            let from = from_pkg.map(|s| s.to_string());

            for value in &removed_values {
                let val_id =
                    unique_id(format!("{}-val-{}", base_id, sanitize_id(value)), id_counts);
                rules.push(KonveyorRule {
                    rule_id: val_id,
                    labels: vec![
                        "source=semver-analyzer".to_string(),
                        "change-type=prop-value-change".to_string(),
                        format!("kind={}", api_kind_label(&change.kind)),
                        "has-codemod=true".to_string(),
                    ],
                    effort: 1,
                    category: "mandatory".to_string(),
                    description: format!("Value '{}' removed from '{}'", value, change.symbol),
                    message: format!(
                        "The value '{}' is no longer accepted for '{}'. \
                         Update to one of the new accepted values.\n\nFile: {}",
                        value, change.symbol, file_path
                    ),
                    links: Vec::new(),
                    when: KonveyorCondition::FrontendReferenced {
                        referenced: FrontendReferencedFields {
                            pattern: format!(
                                "^{}$",
                                regex_escape(extract_leaf_symbol(&change.symbol))
                            ),
                            location: "JSX_PROP".to_string(),
                            component: parent_component.clone(),
                            parent: None,
                            value: Some(format!("^{}$", regex_escape(value))),
                            from: from.clone(),
                        },
                    },
                });
            }
        }
    }

    rules
}

fn behavioral_change_to_rule(
    change: &BehavioralChange,
    file_changes: &FileChanges,
    file_pattern: &str,
    from_pkg: Option<&str>,
    id_counts: &mut HashMap<String, usize>,
) -> KonveyorRule {
    let file_path = file_changes.file.display().to_string();
    // For dotted symbols like "NavList.render", use the component name (first
    // part) for JSX_COMPONENT matching.  The leaf ("render") is the method that
    // changed, but the detection target is the component consumers use in JSX.
    let leaf_symbol = if change.symbol.contains('.') {
        change
            .symbol
            .splitn(2, '.')
            .next()
            .unwrap_or(&change.symbol)
    } else {
        extract_leaf_symbol(&change.symbol)
    };

    let base_id = format!(
        "semver-{}-{}-behavioral",
        sanitize_id(&file_path),
        sanitize_id(&change.symbol),
    );
    let rule_id = unique_id(base_id, id_counts);

    let message = format!(
        "Behavioral change in '{}': {}\n\nFile: {}\nReview all usages to ensure compatibility with the new behavior.",
        change.symbol, change.description, file_path,
    );

    let mut labels = vec![
        "source=semver-analyzer".to_string(),
        "ai-generated".to_string(),
    ];

    // Use the behavioral category for more precise change-type labels
    if let Some(ref cat) = change.category {
        labels.push(format!("change-type={}", behavioral_category_label(cat)));
        // DOM, CSS, a11y, and behavioral changes primarily impact frontend testing
        if matches!(
            cat,
            semver_analyzer_core::BehavioralCategory::DomStructure
                | semver_analyzer_core::BehavioralCategory::CssClass
                | semver_analyzer_core::BehavioralCategory::CssVariable
                | semver_analyzer_core::BehavioralCategory::Accessibility
                | semver_analyzer_core::BehavioralCategory::DataAttribute
        ) {
            labels.push("impact=frontend-testing".to_string());
        }
    } else {
        labels.push("change-type=behavioral".to_string());
    }

    if let Some(pkg) = from_pkg {
        labels.push(format!("package={}", pkg));
    }

    let from = from_pkg.map(|s| s.to_string());

    // Use frontend.referenced when we have a package scope
    let condition = if from.is_some() {
        KonveyorCondition::FrontendReferenced {
            referenced: FrontendReferencedFields {
                pattern: format!("^{}$", regex_escape(leaf_symbol)),
                location: "JSX_COMPONENT".to_string(),
                component: None,
                parent: None,
                value: None,
                from,
            },
        }
    } else {
        let pattern = format!(r"\b{}\b", regex_escape(leaf_symbol));
        KonveyorCondition::FileContent {
            filecontent: FileContentFields {
                pattern,
                file_pattern: file_pattern.to_string(),
            },
        }
    };

    KonveyorRule {
        rule_id,
        labels,
        effort: 3,
        category: "mandatory".to_string(),
        description: change.description.clone(),
        message,
        links: Vec::new(),
        when: condition,
    }
}

fn manifest_change_to_rule(
    change: &ManifestChange,
    file_pattern: &str,
    id_counts: &mut HashMap<String, usize>,
) -> KonveyorRule {
    let change_type_label = manifest_change_type_label(&change.change_type);

    let base_id = format!(
        "semver-manifest-{}-{}",
        sanitize_id(&change.field),
        change_type_label,
    );
    let rule_id = unique_id(base_id, id_counts);

    let category = if change.is_breaking {
        "mandatory"
    } else {
        "optional"
    };

    let effort = manifest_effort(&change.change_type);

    let (condition, message) =
        build_manifest_condition_and_message(change, file_pattern, change_type_label);

    KonveyorRule {
        rule_id,
        labels: vec![
            "source=semver-analyzer".to_string(),
            "change-type=manifest".to_string(),
            format!("manifest-field={}", change.field),
        ],
        effort,
        category: category.to_string(),
        description: change.description.clone(),
        message,
        links: Vec::new(),
        when: condition,
    }
}

// ── Fix guidance generators ─────────────────────────────────────────────

fn api_change_to_fix(
    change: &ApiChange,
    file_changes: &FileChanges,
    rule_id: &str,
    file_pattern: &str,
) -> FixGuidanceEntry {
    let file_path = file_changes.file.display().to_string();
    let leaf_symbol = extract_leaf_symbol(&change.symbol);
    let search_pattern = build_pattern(&change.kind, &change.change, leaf_symbol, &change.before);

    let (strategy, confidence, source, fix_description, replacement) = match change.change {
        ApiChangeType::Renamed => {
            let old_name = change
                .before
                .as_deref()
                .map(|b| extract_leaf_symbol(b).to_string())
                .unwrap_or_else(|| change.symbol.clone());
            let new_name = change
                .after
                .as_deref()
                .map(|a| extract_leaf_symbol(a).to_string())
                .unwrap_or_else(|| change.symbol.clone());

            let desc = format!(
                "Rename all occurrences of '{}' to '{}'.\n\
                 This is a mechanical find-and-replace that can be auto-applied.\n\
                 Search pattern: {} (in {} files)",
                old_name, new_name, search_pattern, file_pattern,
            );
            (
                FixStrategy::Rename,
                FixConfidence::Exact,
                FixSource::Pattern,
                desc,
                Some(new_name),
            )
        }

        ApiChangeType::SignatureChanged => {
            let desc = if let (Some(ref before), Some(ref after)) = (&change.before, &change.after)
            {
                format!(
                    "Update all call sites of '{}' to match the new signature.\n\n\
                     Old signature: {}\n\
                     New signature: {}\n\n\
                     Review each call site and adjust arguments accordingly.\n\
                     {}",
                    change.symbol, before, after, change.description,
                )
            } else {
                format!(
                    "Update all call sites of '{}' to match the new signature.\n\
                     {}\n\n\
                     Review each usage and adjust arguments, type parameters, or \
                     modifiers as described above.",
                    change.symbol, change.description,
                )
            };

            (
                FixStrategy::UpdateSignature,
                FixConfidence::High,
                FixSource::Pattern,
                desc,
                None,
            )
        }

        ApiChangeType::TypeChanged => {
            let desc = if let (Some(ref before), Some(ref after)) = (&change.before, &change.after)
            {
                format!(
                    "Update type annotations from '{}' to '{}'.\n\n\
                     Old type: {}\n\
                     New type: {}\n\n\
                     Check all locations where this type is used in assignments, \
                     function parameters, return types, and generic type arguments.\n\
                     {}",
                    change.symbol, change.symbol, before, after, change.description,
                )
            } else {
                format!(
                    "Update type references for '{}'.\n\
                     {}\n\n\
                     Check all locations where this type is used and update accordingly.",
                    change.symbol, change.description,
                )
            };

            (
                FixStrategy::UpdateType,
                FixConfidence::High,
                FixSource::Pattern,
                desc,
                None,
            )
        }

        ApiChangeType::Removed => {
            let kind_label = api_kind_label(&change.kind);
            let desc = format!(
                "The {} '{}' has been removed.\n\n\
                 Action required:\n\
                 1. Find all usages of '{}' in your codebase\n\
                 2. Identify an appropriate replacement (check the library's \
                    migration guide or changelog)\n\
                 3. Update each usage to use the replacement\n\
                 4. Remove any imports of '{}'\n\n\
                 {}",
                kind_label, change.symbol, change.symbol, change.symbol, change.description,
            );

            (
                FixStrategy::FindAlternative,
                FixConfidence::Low,
                FixSource::Manual,
                desc,
                None,
            )
        }

        ApiChangeType::VisibilityChanged => {
            let desc = format!(
                "The visibility of '{}' has been reduced.\n\n\
                 If you are importing or using '{}' from outside its module, \
                 you need to find a public alternative.\n\
                 {}\n\n\
                 Check if there is a new public API that exposes the same functionality, \
                 or refactor your code to avoid depending on this internal symbol.",
                change.symbol, change.symbol, change.description,
            );

            (
                FixStrategy::FindAlternative,
                FixConfidence::Medium,
                FixSource::Pattern,
                desc,
                None,
            )
        }
    };

    FixGuidanceEntry {
        rule_id: rule_id.to_string(),
        strategy,
        confidence,
        source,
        symbol: change.symbol.clone(),
        file: file_path,
        fix_description,
        before: change.before.clone(),
        after: change.after.clone(),
        search_pattern,
        replacement,
    }
}

fn behavioral_change_to_fix(
    change: &BehavioralChange,
    file_changes: &FileChanges,
    rule_id: &str,
) -> FixGuidanceEntry {
    let file_path = file_changes.file.display().to_string();
    let leaf_symbol = extract_leaf_symbol(&change.symbol);
    let search_pattern = format!(r"\b{}\b", regex_escape(leaf_symbol));

    let fix_description = format!(
        "Behavioral change detected in '{}' (AI-generated finding).\n\n\
         What changed: {}\n\n\
         Action required:\n\
         1. Review all usages of '{}' in your codebase\n\
         2. Verify that your code handles the new behavior correctly\n\
         3. Update tests that depend on the old behavior\n\
         4. Pay special attention to edge cases and error handling\n\n\
         This finding was generated by LLM analysis and should be \
         verified by a developer.",
        change.symbol, change.description, change.symbol,
    );

    FixGuidanceEntry {
        rule_id: rule_id.to_string(),
        strategy: FixStrategy::ManualReview,
        confidence: FixConfidence::Medium,
        source: FixSource::Llm,
        symbol: change.symbol.clone(),
        file: file_path,
        fix_description,
        before: None,
        after: None,
        search_pattern,
        replacement: None,
    }
}

fn manifest_change_to_fix(change: &ManifestChange, rule_id: &str) -> FixGuidanceEntry {
    let (strategy, confidence, source, fix_description, search, replacement) =
        match change.change_type {
            ManifestChangeType::ModuleSystemChanged => {
                let is_cjs_to_esm = change
                    .after
                    .as_deref()
                    .map(|a| a == "module")
                    .unwrap_or(false);

                if is_cjs_to_esm {
                    (
                        FixStrategy::UpdateImport,
                        FixConfidence::High,
                        FixSource::Pattern,
                        format!(
                            "The package has changed from CommonJS to ESM.\n\n\
                             Action required:\n\
                             1. Convert all require() calls to import statements:\n\
                             \n\
                             Before: const {{ foo }} = require('package')\n\
                             After:  import {{ foo }} from 'package'\n\
                             \n\
                             2. Convert module.exports to export statements:\n\
                             \n\
                             Before: module.exports = {{ foo }}\n\
                             After:  export {{ foo }}\n\
                             \n\
                             3. Update your package.json \"type\" field if needed\n\
                             4. Rename .js files to .mjs if mixing module systems\n\n\
                             {}",
                            change.description,
                        ),
                        r"\brequire\s*\(".to_string(),
                        Some("import".to_string()),
                    )
                } else {
                    (
                        FixStrategy::UpdateImport,
                        FixConfidence::High,
                        FixSource::Pattern,
                        format!(
                            "The package has changed from ESM to CommonJS.\n\n\
                             Action required:\n\
                             1. Convert all import statements to require() calls:\n\
                             \n\
                             Before: import {{ foo }} from 'package'\n\
                             After:  const {{ foo }} = require('package')\n\
                             \n\
                             2. Convert export statements to module.exports\n\
                             3. Update your package.json \"type\" field if needed\n\n\
                             {}",
                            change.description,
                        ),
                        r"\bimport\s+".to_string(),
                        Some("require".to_string()),
                    )
                }
            }

            ManifestChangeType::PeerDependencyAdded => (
                FixStrategy::UpdateDependency,
                FixConfidence::Exact,
                FixSource::Pattern,
                format!(
                    "A new peer dependency has been added: '{}'\n\n\
                     Action required:\n\
                     1. Install the peer dependency: npm install {}\n\
                     2. Verify version compatibility with your existing dependencies\n\n\
                     {}",
                    change.field, change.field, change.description,
                ),
                change.field.clone(),
                change.after.clone(),
            ),

            ManifestChangeType::PeerDependencyRemoved => (
                FixStrategy::UpdateDependency,
                FixConfidence::High,
                FixSource::Pattern,
                format!(
                    "Peer dependency '{}' has been removed.\n\n\
                     Action required:\n\
                     1. Check if you still need '{}' as a direct dependency\n\
                     2. If it was only required by this package, you may be able \
                        to remove it\n\
                     3. Verify that removing it doesn't break other dependencies\n\n\
                     {}",
                    change.field, change.field, change.description,
                ),
                change.field.clone(),
                None,
            ),

            ManifestChangeType::PeerDependencyRangeChanged => (
                FixStrategy::UpdateDependency,
                FixConfidence::High,
                FixSource::Pattern,
                format!(
                    "Peer dependency '{}' version range changed.\n\n\
                     Before: {}\n\
                     After:  {}\n\n\
                     Action required:\n\
                     1. Update '{}' to a version that satisfies the new range\n\
                     2. Test for compatibility with the new version\n\n\
                     {}",
                    change.field,
                    change.before.as_deref().unwrap_or("(none)"),
                    change.after.as_deref().unwrap_or("(none)"),
                    change.field,
                    change.description,
                ),
                change.field.clone(),
                change.after.clone(),
            ),

            ManifestChangeType::EntryPointChanged | ManifestChangeType::ExportsEntryRemoved => (
                FixStrategy::UpdateImport,
                FixConfidence::Medium,
                FixSource::Pattern,
                format!(
                    "Package entry point or export map changed for '{}'.\n\n\
                     Before: {}\n\
                     After:  {}\n\n\
                     Action required:\n\
                     1. Update all import paths that reference the old entry point\n\
                     2. Check the package's export map for the new path\n\n\
                     {}",
                    change.field,
                    change.before.as_deref().unwrap_or("(none)"),
                    change.after.as_deref().unwrap_or("(none)"),
                    change.description,
                ),
                change.field.clone(),
                change.after.clone(),
            ),

            _ => (
                FixStrategy::ManualReview,
                FixConfidence::Medium,
                FixSource::Pattern,
                format!(
                    "Package manifest field '{}' changed.\n\n\
                     Before: {}\n\
                     After:  {}\n\n\
                     Review the change and update your configuration accordingly.\n\n\
                     {}",
                    change.field,
                    change.before.as_deref().unwrap_or("(none)"),
                    change.after.as_deref().unwrap_or("(none)"),
                    change.description,
                ),
                change.field.clone(),
                None,
            ),
        };

    FixGuidanceEntry {
        rule_id: rule_id.to_string(),
        strategy,
        confidence,
        source,
        symbol: change.field.clone(),
        file: "package.json".to_string(),
        fix_description,
        before: change.before.clone(),
        after: change.after.clone(),
        search_pattern: search,
        replacement,
    }
}

// ── Pattern building ────────────────────────────────────────────────────

/// Build a regex pattern for detecting usage of a changed symbol.
///
/// The pattern varies by the kind of symbol and the type of change:
/// - functions/methods: `\bname\s*\(` to match call sites
/// - properties/fields: `\.name\b` to match property access
/// - classes/interfaces/types: `\bname\b` to match any reference
/// - renamed symbols: match the OLD name from `before`
fn build_pattern(
    kind: &ApiChangeKind,
    change: &ApiChangeType,
    leaf_symbol: &str,
    before: &Option<String>,
) -> String {
    // For renames, match the old name
    let name = if *change == ApiChangeType::Renamed {
        if let Some(ref before_val) = before {
            // before might be a full signature; extract just the symbol name
            extract_leaf_symbol(before_val)
        } else {
            leaf_symbol
        }
    } else {
        leaf_symbol
    };

    let escaped = regex_escape(name);

    match kind {
        ApiChangeKind::Function | ApiChangeKind::Method => {
            format!(r"\b{}\s*\(", escaped)
        }
        ApiChangeKind::Property | ApiChangeKind::Field => {
            format!(r"\.{}\b", escaped)
        }
        _ => {
            // class, interface, type_alias, constant, struct, trait, module_export
            format!(r"\b{}\b", escaped)
        }
    }
}

/// Build a `frontend.referenced` condition for an API change.
///
/// Maps `ApiChangeKind` to the appropriate `location` discriminator
/// and extracts `component` filter for property-level changes.
///
/// For renames, generates an `or:` condition matching both JSX_COMPONENT
/// and IMPORT locations (same pattern as hand-crafted rules).
fn build_frontend_condition(
    change: &ApiChange,
    leaf_symbol: &str,
    from_pkg: Option<&str>,
) -> KonveyorCondition {
    // For renames, match the OLD name
    let match_name = if change.change == ApiChangeType::Renamed {
        change
            .before
            .as_deref()
            .map(|b| extract_leaf_symbol(b))
            .unwrap_or(leaf_symbol)
    } else {
        leaf_symbol
    };

    let pattern = format!("^{}$", regex_escape(match_name));
    let from = from_pkg.map(|s| s.to_string());

    // Extract parent component for property/field changes
    // e.g., "Card.isFlat" → component="Card", prop="isFlat"
    let parent_component = if change.symbol.contains('.') {
        let parts: Vec<&str> = change.symbol.splitn(2, '.').collect();
        Some(format!("^{}$", regex_escape(parts[0])))
    } else {
        None
    };

    // When `from` is a regex-anchored sub-path (e.g., `^@pkg/deprecated$`),
    // the provider's `from` filter only works for IMPORT-location incidents
    // because JSX_COMPONENT/JSX_PROP/TYPE_REFERENCE incidents don't carry a
    // `module` variable and are always kept.  In that case we restrict to
    // IMPORT-only to avoid false positives from non-deprecated usages of the
    // same component name.
    let is_subpath_scoped = from
        .as_ref()
        .map_or(false, |f| f.starts_with('^') && f.ends_with('$'));

    match change.kind {
        // Class/Interface used as JSX component → match both JSX and IMPORT
        ApiChangeKind::Class | ApiChangeKind::Interface
            if change.change == ApiChangeType::Renamed =>
        {
            let mut conditions = vec![KonveyorCondition::FrontendReferenced {
                referenced: FrontendReferencedFields {
                    pattern: pattern.clone(),
                    location: "IMPORT".to_string(),
                    component: None,
                    parent: None,
                    value: None,
                    from: from.clone(),
                },
            }];
            if !is_subpath_scoped {
                conditions.insert(
                    0,
                    KonveyorCondition::FrontendReferenced {
                        referenced: FrontendReferencedFields {
                            pattern: pattern.clone(),
                            location: "JSX_COMPONENT".to_string(),
                            component: None,
                            parent: None,
                            value: None,
                            from: from.clone(),
                        },
                    },
                );
            }
            KonveyorCondition::Or { or: conditions }
        }

        // Class/Interface removal/change → match both JSX_COMPONENT and IMPORT
        // Interfaces are imported as types (not used as JSX tags), so IMPORT is
        // essential for detecting removed interfaces like SelectProps, DropdownItemProps.
        //
        // When a *Props interface is removed, also match the component name
        // (without "Props" suffix) at IMPORT. This is a standard React convention:
        // FooProps always accompanies Foo, and consumers import the component.
        ApiChangeKind::Class | ApiChangeKind::Interface => {
            let mut conditions = Vec::new();
            if !is_subpath_scoped {
                conditions.push(KonveyorCondition::FrontendReferenced {
                    referenced: FrontendReferencedFields {
                        pattern: pattern.clone(),
                        location: "JSX_COMPONENT".to_string(),
                        component: None,
                        parent: None,
                        value: None,
                        from: from.clone(),
                    },
                });
            }
            conditions.push(KonveyorCondition::FrontendReferenced {
                referenced: FrontendReferencedFields {
                    pattern: pattern.clone(),
                    location: "IMPORT".to_string(),
                    component: None,
                    parent: None,
                    value: None,
                    from: from.clone(),
                },
            });
            // If FooProps → also match Foo at IMPORT
            if match_name.ends_with("Props") {
                let component_name = &match_name[..match_name.len() - 5];
                if !component_name.is_empty() {
                    let comp_pattern = format!("^{}$", regex_escape(component_name));
                    conditions.push(KonveyorCondition::FrontendReferenced {
                        referenced: FrontendReferencedFields {
                            pattern: comp_pattern,
                            location: "IMPORT".to_string(),
                            component: None,
                            parent: None,
                            value: None,
                            from: from.clone(),
                        },
                    });
                }
            }
            KonveyorCondition::Or { or: conditions }
        }

        // Property/Field → match as JSX prop, scoped to parent component
        // If this is a union member removal (before='value'), add a value filter.
        // For sub-path scoped rules, fall back to IMPORT since JSX_PROP can't
        // be filtered by package.
        ApiChangeKind::Property | ApiChangeKind::Field => {
            let value_filter = extract_value_filter(change);
            if is_subpath_scoped {
                KonveyorCondition::FrontendReferenced {
                    referenced: FrontendReferencedFields {
                        pattern,
                        location: "IMPORT".to_string(),
                        component: None,
                        parent: None,
                        value: None,
                        from,
                    },
                }
            } else {
                KonveyorCondition::FrontendReferenced {
                    referenced: FrontendReferencedFields {
                        pattern,
                        location: "JSX_PROP".to_string(),
                        component: parent_component,
                        parent: None,
                        value: value_filter,
                        from,
                    },
                }
            }
        }

        // Function/Method → match as function call
        ApiChangeKind::Function | ApiChangeKind::Method => KonveyorCondition::FrontendReferenced {
            referenced: FrontendReferencedFields {
                pattern,
                location: if is_subpath_scoped {
                    "IMPORT".to_string()
                } else {
                    "FUNCTION_CALL".to_string()
                },
                component: None,
                parent: None,
                value: None,
                from,
            },
        },

        // TypeAlias → match as both TYPE_REFERENCE and IMPORT.
        // TYPE_REFERENCE catches usages like `const x: FooType = ...`.
        // IMPORT catches the import statement that needs updating when the
        // type alias is renamed or removed.
        // For sub-path scoped rules, skip TYPE_REFERENCE (can't filter by package).
        ApiChangeKind::TypeAlias => {
            let mut conditions = Vec::new();
            if !is_subpath_scoped {
                conditions.push(KonveyorCondition::FrontendReferenced {
                    referenced: FrontendReferencedFields {
                        pattern: pattern.clone(),
                        location: "TYPE_REFERENCE".to_string(),
                        component: None,
                        parent: None,
                        value: None,
                        from: from.clone(),
                    },
                });
            }
            conditions.push(KonveyorCondition::FrontendReferenced {
                referenced: FrontendReferencedFields {
                    pattern,
                    location: "IMPORT".to_string(),
                    component: None,
                    parent: None,
                    value: None,
                    from,
                },
            });
            if conditions.len() == 1 {
                conditions.into_iter().next().unwrap()
            } else {
                KonveyorCondition::Or { or: conditions }
            }
        }

        // Constants, module exports, structs, traits → match as import.
        // PascalCase constants are React component functions (e.g., DropdownItem,
        // Chart, Select) used as `<Component>` in JSX.  Emit both JSX_COMPONENT
        // and IMPORT so the rule matches at JSX usage sites, not just the import.
        // For sub-path scoped rules, skip JSX_COMPONENT (can't filter by package).
        _ => {
            let is_component = match_name
                .chars()
                .next()
                .map_or(false, |c| c.is_ascii_uppercase());
            if is_component && !is_subpath_scoped {
                KonveyorCondition::Or {
                    or: vec![
                        KonveyorCondition::FrontendReferenced {
                            referenced: FrontendReferencedFields {
                                pattern: pattern.clone(),
                                location: "JSX_COMPONENT".to_string(),
                                component: None,
                                parent: None,
                                value: None,
                                from: from.clone(),
                            },
                        },
                        KonveyorCondition::FrontendReferenced {
                            referenced: FrontendReferencedFields {
                                pattern,
                                location: "IMPORT".to_string(),
                                component: None,
                                parent: None,
                                value: None,
                                from,
                            },
                        },
                    ],
                }
            } else {
                KonveyorCondition::FrontendReferenced {
                    referenced: FrontendReferencedFields {
                        pattern,
                        location: "IMPORT".to_string(),
                        component: None,
                        parent: None,
                        value: None,
                        from,
                    },
                }
            }
        }
    }
}

/// Extract a value filter from an ApiChange if it represents a single union
/// member removal (e.g., `before: "'tertiary'"`).
fn extract_value_filter(change: &ApiChange) -> Option<String> {
    let before = change.before.as_deref()?;
    // Must be a single quoted value — NOT a union like `'a' | 'b'`
    if is_single_quoted_value(before) {
        let value = &before[1..before.len() - 1];
        if !value.is_empty() {
            return Some(format!("^{}$", regex_escape(value)));
        }
    }
    None
}

/// Check if a string is a single quoted value (not a union).
fn is_single_quoted_value(s: &str) -> bool {
    let s = s.trim();
    if s.len() < 2 {
        return false;
    }
    let quote = s.as_bytes()[0];
    if quote != b'\'' && quote != b'"' {
        return false;
    }
    // Must start and end with same quote, and contain no `|` outside the quotes
    if s.as_bytes()[s.len() - 1] != quote {
        return false;
    }
    // Check for union pipe — if there's a `|` between quotes, it's a union
    let inner = &s[1..s.len() - 1];
    !inner.contains(" | ") && !inner.contains('|')
}

/// Parse string literal union members from a type expression.
///
/// Handles both simple unions (`'a' | 'b' | 'c'`) and unions embedded in
/// object types (`{ default?: 'alignLeft' | 'alignRight'; ... }`).
///
/// Returns a set of the extracted string literal values.
fn parse_union_string_values(type_expr: &str) -> BTreeSet<String> {
    let mut values = BTreeSet::new();
    // Use a simple regex to extract all single-quoted string literals
    let re = regex::Regex::new(r"'([^']+)'").unwrap();
    for cap in re.captures_iter(type_expr) {
        values.insert(cap[1].to_string());
    }
    values
}

/// Compute the removed union member values between before and after type
/// expressions.  Returns values present in `before` but missing from `after`.
fn extract_removed_union_values(change: &ApiChange) -> Vec<String> {
    let before = match change.before.as_deref() {
        Some(b) => b,
        None => return Vec::new(),
    };
    let after = match change.after.as_deref() {
        Some(a) => a,
        None => return Vec::new(),
    };
    // Only apply to type_changed — for removed props the whole prop is gone
    if change.change != ApiChangeType::TypeChanged {
        return Vec::new();
    }
    let before_vals = parse_union_string_values(before);
    let after_vals = parse_union_string_values(after);
    // Must have at least 2 values in before to be a union worth splitting
    if before_vals.len() < 2 {
        return Vec::new();
    }
    before_vals.difference(&after_vals).cloned().collect()
}

/// Build the condition and message for a manifest change.
fn build_manifest_condition_and_message(
    change: &ManifestChange,
    file_pattern: &str,
    change_type_label: &str,
) -> (KonveyorCondition, String) {
    match change.change_type {
        ManifestChangeType::ModuleSystemChanged => {
            let is_cjs_to_esm = change
                .after
                .as_deref()
                .map(|a| a == "module")
                .unwrap_or(false);

            let (pattern, hint) = if is_cjs_to_esm {
                (
                    r"\brequire\s*\(".to_string(),
                    "Convert require() calls to ESM import statements.",
                )
            } else {
                (
                    r"\bimport\s+".to_string(),
                    "Convert ESM import statements to require() calls.",
                )
            };

            let message = format!(
                "Module system changed: {}\n\nBefore: {}\nAfter: {}\n{}",
                change.description,
                change.before.as_deref().unwrap_or("(none)"),
                change.after.as_deref().unwrap_or("(none)"),
                hint,
            );

            (
                KonveyorCondition::FileContent {
                    filecontent: FileContentFields {
                        pattern,
                        file_pattern: file_pattern.to_string(),
                    },
                },
                message,
            )
        }
        ManifestChangeType::PeerDependencyAdded
        | ManifestChangeType::PeerDependencyRemoved
        | ManifestChangeType::PeerDependencyRangeChanged => {
            let message = format!(
                "Peer dependency change ({}): {}\n\nField: {}\nBefore: {}\nAfter: {}",
                change_type_label,
                change.description,
                change.field,
                change.before.as_deref().unwrap_or("(none)"),
                change.after.as_deref().unwrap_or("(none)"),
            );

            (
                KonveyorCondition::Json {
                    json: JsonFields {
                        xpath: format!("//peerDependencies/{}", change.field),
                        filepaths: Some("package.json".to_string()),
                    },
                },
                message,
            )
        }
        _ => {
            // Generic manifest change: use filecontent to match the field name
            let message = format!(
                "Package manifest change ({}): {}\n\nField: {}\nBefore: {}\nAfter: {}",
                change_type_label,
                change.description,
                change.field,
                change.before.as_deref().unwrap_or("(none)"),
                change.after.as_deref().unwrap_or("(none)"),
            );

            (
                KonveyorCondition::Json {
                    json: JsonFields {
                        xpath: format!("//{}", change.field),
                        filepaths: Some("package.json".to_string()),
                    },
                },
                message,
            )
        }
    }
}

// ── Message building ────────────────────────────────────────────────────

fn build_api_message(change: &ApiChange, file_path: &str) -> String {
    let change_verb = match change.change {
        ApiChangeType::Removed => "was removed",
        ApiChangeType::SignatureChanged => "had its signature changed",
        ApiChangeType::TypeChanged => "had its type changed",
        ApiChangeType::VisibilityChanged => "had its visibility changed",
        ApiChangeType::Renamed => "was renamed",
    };

    let kind_label = api_kind_label(&change.kind);

    let mut msg = format!(
        "{} '{}' {} ({}): {}\n\nFile: {}",
        capitalize(kind_label),
        change.symbol,
        change_verb,
        kind_label,
        change.description,
        file_path,
    );

    if let Some(ref before) = change.before {
        msg.push_str(&format!("\nBefore: {}", before));
    }
    if let Some(ref after) = change.after {
        msg.push_str(&format!("\nAfter: {}", after));
    }

    msg
}

// ── Effort mapping ──────────────────────────────────────────────────────

fn effort_for_api_change(change: &ApiChangeType) -> u32 {
    match change {
        ApiChangeType::Removed => 5,
        ApiChangeType::SignatureChanged => 3,
        ApiChangeType::TypeChanged => 3,
        ApiChangeType::VisibilityChanged => 3,
        ApiChangeType::Renamed => 1,
    }
}

fn manifest_effort(change_type: &ManifestChangeType) -> u32 {
    match change_type {
        ManifestChangeType::ModuleSystemChanged => 7,
        ManifestChangeType::EntryPointChanged => 5,
        ManifestChangeType::ExportsEntryRemoved => 5,
        ManifestChangeType::ExportsConditionRemoved => 3,
        ManifestChangeType::BinEntryRemoved => 3,
        _ => 3,
    }
}

// ── Label helpers ───────────────────────────────────────────────────────

fn api_change_type_label(change: &ApiChangeType) -> &'static str {
    match change {
        ApiChangeType::Removed => "removed",
        ApiChangeType::SignatureChanged => "signature-changed",
        ApiChangeType::TypeChanged => "type-changed",
        ApiChangeType::VisibilityChanged => "visibility-changed",
        ApiChangeType::Renamed => "renamed",
    }
}

fn api_kind_label(kind: &ApiChangeKind) -> &'static str {
    match kind {
        ApiChangeKind::Function => "function",
        ApiChangeKind::Method => "method",
        ApiChangeKind::Class => "class",
        ApiChangeKind::Struct => "struct",
        ApiChangeKind::Interface => "interface",
        ApiChangeKind::Trait => "trait",
        ApiChangeKind::TypeAlias => "type-alias",
        ApiChangeKind::Constant => "constant",
        ApiChangeKind::Field => "field",
        ApiChangeKind::Property => "property",
        ApiChangeKind::ModuleExport => "module-export",
    }
}

fn behavioral_category_label(cat: &semver_analyzer_core::BehavioralCategory) -> &'static str {
    use semver_analyzer_core::BehavioralCategory;
    match cat {
        BehavioralCategory::DomStructure => "dom-structure",
        BehavioralCategory::CssClass => "css-class",
        BehavioralCategory::CssVariable => "css-variable",
        BehavioralCategory::Accessibility => "accessibility",
        BehavioralCategory::DefaultValue => "default-value",
        BehavioralCategory::LogicChange => "logic-change",
        BehavioralCategory::DataAttribute => "data-attribute",
        BehavioralCategory::RenderOutput => "render-output",
    }
}

fn manifest_change_type_label(change_type: &ManifestChangeType) -> &'static str {
    match change_type {
        ManifestChangeType::EntryPointChanged => "entry-point-changed",
        ManifestChangeType::ExportsEntryRemoved => "exports-entry-removed",
        ManifestChangeType::ExportsEntryAdded => "exports-entry-added",
        ManifestChangeType::ExportsConditionRemoved => "exports-condition-removed",
        ManifestChangeType::ModuleSystemChanged => "module-system-changed",
        ManifestChangeType::PeerDependencyAdded => "peer-dependency-added",
        ManifestChangeType::PeerDependencyRemoved => "peer-dependency-removed",
        ManifestChangeType::PeerDependencyRangeChanged => "peer-dependency-range-changed",
        ManifestChangeType::EngineConstraintChanged => "engine-constraint-changed",
        ManifestChangeType::BinEntryRemoved => "bin-entry-removed",
    }
}

// ── Utility helpers ─────────────────────────────────────────────────────

/// Extract the leaf symbol name from a potentially dotted path.
/// e.g. "Card.isFlat" → "isFlat", "createUser" → "createUser"
fn extract_leaf_symbol(symbol: &str) -> &str {
    symbol.rsplit('.').next().unwrap_or(symbol)
}

/// Sanitize a string for use in a Konveyor rule ID.
/// Replaces non-alphanumeric characters with hyphens, lowercases, and deduplicates.
fn sanitize_id(s: &str) -> String {
    let sanitized: String = s
        .chars()
        .map(|c| {
            if c.is_ascii_alphanumeric() {
                c.to_ascii_lowercase()
            } else {
                '-'
            }
        })
        .collect();

    // Collapse consecutive hyphens and trim
    let mut result = String::with_capacity(sanitized.len());
    let mut prev_hyphen = false;
    for ch in sanitized.chars() {
        if ch == '-' {
            if !prev_hyphen && !result.is_empty() {
                result.push('-');
            }
            prev_hyphen = true;
        } else {
            result.push(ch);
            prev_hyphen = false;
        }
    }
    // Trim trailing hyphen
    if result.ends_with('-') {
        result.pop();
    }

    result
}

/// Generate a unique rule ID by appending a counter for duplicates.
fn unique_id(base: String, counts: &mut HashMap<String, usize>) -> String {
    let count = counts.entry(base.clone()).or_insert(0);
    *count += 1;
    if *count == 1 {
        base
    } else {
        format!("{}-{}", base, count)
    }
}

/// Escape special regex characters in a symbol name.
fn regex_escape(s: &str) -> String {
    let mut escaped = String::with_capacity(s.len());
    for c in s.chars() {
        match c {
            '.' | '*' | '+' | '?' | '(' | ')' | '[' | ']' | '{' | '}' | '|' | '^' | '$' | '\\' => {
                escaped.push('\\');
                escaped.push(c);
            }
            _ => escaped.push(c),
        }
    }
    escaped
}

fn capitalize(s: &str) -> String {
    let mut chars = s.chars();
    match chars.next() {
        None => String::new(),
        Some(first) => {
            let upper: String = first.to_uppercase().collect();
            upper + chars.as_str()
        }
    }
}

// ── Tests ───────────────────────────────────────────────────────────────

#[cfg(test)]
mod tests {
    use super::*;
    use semver_analyzer_core::*;
    use std::path::PathBuf;

    fn make_report(
        changes: Vec<FileChanges>,
        manifest_changes: Vec<ManifestChange>,
    ) -> AnalysisReport {
        AnalysisReport {
            repository: PathBuf::from("/tmp/test-repo"),
            comparison: Comparison {
                from_ref: "v1.0.0".to_string(),
                to_ref: "v2.0.0".to_string(),
                from_sha: "abc123".to_string(),
                to_sha: "def456".to_string(),
                commit_count: 10,
                analysis_timestamp: "2026-03-16T00:00:00Z".to_string(),
            },
            summary: Summary {
                total_breaking_changes: 0,
                breaking_api_changes: 0,
                breaking_behavioral_changes: 0,
                files_with_breaking_changes: 0,
            },
            changes,
            manifest_changes,
            metadata: AnalysisMetadata {
                call_graph_analysis: "none".to_string(),
                tool_version: "0.1.0".to_string(),
                llm_usage: None,
            },
        }
    }

    #[test]
    fn test_extract_leaf_symbol() {
        assert_eq!(extract_leaf_symbol("Card.isFlat"), "isFlat");
        assert_eq!(extract_leaf_symbol("createUser"), "createUser");
        assert_eq!(extract_leaf_symbol("a.b.c"), "c");
    }

    #[test]
    fn test_sanitize_id() {
        assert_eq!(sanitize_id("src/api/users.d.ts"), "src-api-users-d-ts");
        assert_eq!(sanitize_id("Card.isFlat"), "card-isflat");
        assert_eq!(sanitize_id("foo///bar"), "foo-bar");
    }

    #[test]
    fn test_unique_id() {
        let mut counts = HashMap::new();
        assert_eq!(unique_id("foo".to_string(), &mut counts), "foo");
        assert_eq!(unique_id("foo".to_string(), &mut counts), "foo-2");
        assert_eq!(unique_id("foo".to_string(), &mut counts), "foo-3");
        assert_eq!(unique_id("bar".to_string(), &mut counts), "bar");
    }

    #[test]
    fn test_regex_escape() {
        assert_eq!(regex_escape("foo"), "foo");
        assert_eq!(regex_escape("foo.bar"), "foo\\.bar");
        assert_eq!(regex_escape("a*b+c?"), "a\\*b\\+c\\?");
    }

    #[test]
    fn test_build_pattern_function_removed() {
        let pattern = build_pattern(
            &ApiChangeKind::Function,
            &ApiChangeType::Removed,
            "createUser",
            &None,
        );
        assert_eq!(pattern, r"\bcreateUser\s*\(");
    }

    #[test]
    fn test_build_pattern_property_removed() {
        let pattern = build_pattern(
            &ApiChangeKind::Property,
            &ApiChangeType::Removed,
            "isFlat",
            &None,
        );
        assert_eq!(pattern, r"\.isFlat\b");
    }

    #[test]
    fn test_build_pattern_class_removed() {
        let pattern = build_pattern(
            &ApiChangeKind::Class,
            &ApiChangeType::Removed,
            "Card",
            &None,
        );
        assert_eq!(pattern, r"\bCard\b");
    }

    #[test]
    fn test_build_pattern_renamed_uses_before() {
        let pattern = build_pattern(
            &ApiChangeKind::Function,
            &ApiChangeType::Renamed,
            "newName",
            &Some("oldName".to_string()),
        );
        // Should match the OLD name, not the new one
        assert_eq!(pattern, r"\boldName\s*\(");
    }

    #[test]
    fn test_generate_rules_api_change() {
        let changes = vec![FileChanges {
            file: PathBuf::from("src/api/users.d.ts"),
            status: FileStatus::Modified,
            renamed_from: None,
            breaking_api_changes: vec![ApiChange {
                symbol: "createUser".to_string(),
                kind: ApiChangeKind::Function,
                change: ApiChangeType::Removed,
                before: None,
                after: None,
                description: "Exported function 'createUser' was removed".to_string(),
            }],
            breaking_behavioral_changes: vec![],
        }];

        let report = make_report(changes, vec![]);
        let empty_cache = HashMap::new();
        let rules = generate_rules(
            &report,
            "*.{ts,tsx,js,jsx}",
            &empty_cache,
            &RenamePatterns::empty(),
        );

        assert_eq!(rules.len(), 1);
        assert_eq!(
            rules[0].rule_id,
            "semver-src-api-users-d-ts-createuser-removed"
        );
        assert_eq!(rules[0].category, "mandatory");
        assert_eq!(rules[0].effort, 5);
        assert!(rules[0]
            .labels
            .contains(&"source=semver-analyzer".to_string()));
        assert!(rules[0].labels.contains(&"change-type=removed".to_string()));
        assert!(rules[0].labels.contains(&"kind=function".to_string()));
    }

    #[test]
    fn test_generate_rules_behavioral_change() {
        let changes = vec![FileChanges {
            file: PathBuf::from("src/api/users.ts"),
            status: FileStatus::Modified,
            renamed_from: None,
            breaking_api_changes: vec![],
            breaking_behavioral_changes: vec![BehavioralChange {
                symbol: "validateEmail".to_string(),
                kind: BehavioralChangeKind::Function,
                category: None,
                description: "Now rejects emails with '+' aliases".to_string(),
                source_file: Some("src/api/users.ts".to_string()),
            }],
        }];

        let report = make_report(changes, vec![]);
        let empty_cache = HashMap::new();
        let rules = generate_rules(
            &report,
            "*.{ts,tsx}",
            &empty_cache,
            &RenamePatterns::empty(),
        );

        assert_eq!(rules.len(), 1);
        assert!(rules[0].rule_id.contains("behavioral"));
        assert_eq!(rules[0].category, "mandatory");
        assert!(rules[0].labels.contains(&"ai-generated".to_string()));
        assert!(rules[0]
            .labels
            .contains(&"change-type=behavioral".to_string()));
    }

    #[test]
    fn test_generate_rules_manifest_module_system() {
        let manifest = vec![ManifestChange {
            field: "type".to_string(),
            change_type: ManifestChangeType::ModuleSystemChanged,
            before: Some("commonjs".to_string()),
            after: Some("module".to_string()),
            description: "CJS to ESM".to_string(),
            is_breaking: true,
        }];

        let report = make_report(vec![], manifest);
        let empty_cache = HashMap::new();
        let rules = generate_rules(
            &report,
            "*.{ts,tsx,js,jsx}",
            &empty_cache,
            &RenamePatterns::empty(),
        );

        assert_eq!(rules.len(), 1);
        assert!(rules[0].rule_id.contains("manifest"));
        assert!(rules[0].rule_id.contains("module-system-changed"));
        assert_eq!(rules[0].category, "mandatory");
        assert_eq!(rules[0].effort, 7);

        // Should use filecontent to match require() calls
        match &rules[0].when {
            KonveyorCondition::FileContent { filecontent } => {
                assert!(filecontent.pattern.contains("require"));
            }
            _ => panic!("Expected FileContent condition for module system change"),
        }
    }

    #[test]
    fn test_generate_rules_manifest_peer_dep() {
        let manifest = vec![ManifestChange {
            field: "react".to_string(),
            change_type: ManifestChangeType::PeerDependencyRemoved,
            before: Some("^17.0.0".to_string()),
            after: None,
            description: "Peer dependency 'react' was removed".to_string(),
            is_breaking: true,
        }];

        let report = make_report(vec![], manifest);
        let empty_cache = HashMap::new();
        let rules = generate_rules(
            &report,
            "*.{ts,tsx,js,jsx}",
            &empty_cache,
            &RenamePatterns::empty(),
        );

        assert_eq!(rules.len(), 1);
        // Should use builtin.json condition
        match &rules[0].when {
            KonveyorCondition::Json { json } => {
                assert!(json.xpath.contains("peerDependencies"));
            }
            _ => panic!("Expected Json condition for peer dependency change"),
        }
    }

    #[test]
    fn test_duplicate_rule_ids_get_suffix() {
        let changes = vec![FileChanges {
            file: PathBuf::from("test.d.ts"),
            status: FileStatus::Modified,
            renamed_from: None,
            breaking_api_changes: vec![
                ApiChange {
                    symbol: "foo".to_string(),
                    kind: ApiChangeKind::Function,
                    change: ApiChangeType::Removed,
                    before: None,
                    after: None,
                    description: "Removed foo".to_string(),
                },
                ApiChange {
                    symbol: "foo".to_string(),
                    kind: ApiChangeKind::Function,
                    change: ApiChangeType::Removed,
                    before: None,
                    after: None,
                    description: "Removed foo overload".to_string(),
                },
            ],
            breaking_behavioral_changes: vec![],
        }];

        let report = make_report(changes, vec![]);
        let empty_cache = HashMap::new();
        let rules = generate_rules(&report, "*.ts", &empty_cache, &RenamePatterns::empty());

        assert_eq!(rules.len(), 2);
        assert_ne!(rules[0].rule_id, rules[1].rule_id);
        assert!(rules[1].rule_id.ends_with("-2"));
    }

    #[test]
    fn test_write_ruleset_dir() {
        let base = std::env::temp_dir().join("semver-konveyor-test-out");
        let dir = base.join("rules");
        let _ = std::fs::remove_dir_all(&base);

        let report = make_report(vec![], vec![]);
        let empty_cache = HashMap::new();
        let rules = generate_rules(&report, "*.ts", &empty_cache, &RenamePatterns::empty());
        let fix_guidance = generate_fix_guidance(&report, &rules, "*.ts");

        write_ruleset_dir(&dir, "test-ruleset", &report, &rules).unwrap();
        let fix_dir = write_fix_guidance_dir(&dir, &fix_guidance).unwrap();

        // Ruleset dir contains rules only
        assert!(dir.join("ruleset.yaml").exists());
        assert!(dir.join("breaking-changes.yaml").exists());
        assert!(!dir.join("fix-guidance.yaml").exists()); // NOT in rules dir

        // Fix guidance is in sibling directory
        assert_eq!(fix_dir, base.join("fix-guidance"));
        assert!(fix_dir.join("fix-guidance.yaml").exists());

        let ruleset_content = std::fs::read_to_string(dir.join("ruleset.yaml")).unwrap();
        assert!(ruleset_content.contains("test-ruleset"));
        assert!(ruleset_content.contains("source=semver-analyzer"));

        let fix_content = std::fs::read_to_string(fix_dir.join("fix-guidance.yaml")).unwrap();
        assert!(fix_content.contains("migration"));
        assert!(fix_content.contains("total_fixes"));

        let _ = std::fs::remove_dir_all(&base);
    }

    #[test]
    fn test_full_roundtrip_yaml_output() {
        let changes = vec![FileChanges {
            file: PathBuf::from("src/components/Button.d.ts"),
            status: FileStatus::Modified,
            renamed_from: None,
            breaking_api_changes: vec![ApiChange {
                symbol: "Button.variant".to_string(),
                kind: ApiChangeKind::Property,
                change: ApiChangeType::TypeChanged,
                before: Some("'primary' | 'secondary'".to_string()),
                after: Some("'primary' | 'danger'".to_string()),
                description: "Removed 'secondary' variant, added 'danger'".to_string(),
            }],
            breaking_behavioral_changes: vec![],
        }];

        let report = make_report(changes, vec![]);
        let empty_cache = HashMap::new();
        let rules = generate_rules(
            &report,
            "*.{ts,tsx}",
            &empty_cache,
            &RenamePatterns::empty(),
        );

        // Verify YAML serialization succeeds
        let yaml = serde_yaml::to_string(&rules).unwrap();
        assert!(yaml.contains("ruleID"));
        assert!(yaml.contains("frontend.referenced"));
        assert!(yaml.contains("variant"));
    }

    // ── Fix guidance tests ──────────────────────────────────────────────

    #[test]
    fn test_fix_guidance_renamed_is_exact() {
        let changes = vec![FileChanges {
            file: PathBuf::from("src/lib.d.ts"),
            status: FileStatus::Modified,
            renamed_from: None,
            breaking_api_changes: vec![ApiChange {
                symbol: "Chip".to_string(),
                kind: ApiChangeKind::Class,
                change: ApiChangeType::Renamed,
                before: Some("Chip".to_string()),
                after: Some("Label".to_string()),
                description: "Chip renamed to Label".to_string(),
            }],
            breaking_behavioral_changes: vec![],
        }];

        let report = make_report(changes, vec![]);
        let empty_cache = HashMap::new();
        let rules = generate_rules(
            &report,
            "*.{ts,tsx}",
            &empty_cache,
            &RenamePatterns::empty(),
        );
        let guidance = generate_fix_guidance(&report, &rules, "*.{ts,tsx}");

        assert_eq!(guidance.fixes.len(), 1);
        let fix = &guidance.fixes[0];
        assert!(matches!(fix.strategy, FixStrategy::Rename));
        assert!(matches!(fix.confidence, FixConfidence::Exact));
        assert!(matches!(fix.source, FixSource::Pattern));
        assert_eq!(fix.replacement.as_deref(), Some("Label"));
        assert!(fix.fix_description.contains("Rename all occurrences"));
        assert!(fix.fix_description.contains("'Chip'"));
        assert!(fix.fix_description.contains("'Label'"));
    }

    #[test]
    fn test_fix_guidance_removed_is_manual() {
        let changes = vec![FileChanges {
            file: PathBuf::from("src/api.d.ts"),
            status: FileStatus::Deleted,
            renamed_from: None,
            breaking_api_changes: vec![ApiChange {
                symbol: "createUser".to_string(),
                kind: ApiChangeKind::Function,
                change: ApiChangeType::Removed,
                before: None,
                after: None,
                description: "Function createUser was removed".to_string(),
            }],
            breaking_behavioral_changes: vec![],
        }];

        let report = make_report(changes, vec![]);
        let empty_cache = HashMap::new();
        let rules = generate_rules(&report, "*.ts", &empty_cache, &RenamePatterns::empty());
        let guidance = generate_fix_guidance(&report, &rules, "*.ts");

        assert_eq!(guidance.fixes.len(), 1);
        let fix = &guidance.fixes[0];
        assert!(matches!(fix.strategy, FixStrategy::FindAlternative));
        assert!(matches!(fix.confidence, FixConfidence::Low));
        assert!(matches!(fix.source, FixSource::Manual));
        assert!(fix.replacement.is_none());
        assert!(fix.fix_description.contains("has been removed"));
    }

    #[test]
    fn test_fix_guidance_signature_changed() {
        let changes = vec![FileChanges {
            file: PathBuf::from("src/utils.d.ts"),
            status: FileStatus::Modified,
            renamed_from: None,
            breaking_api_changes: vec![ApiChange {
                symbol: "formatDate".to_string(),
                kind: ApiChangeKind::Function,
                change: ApiChangeType::SignatureChanged,
                before: Some("formatDate(d: Date): string".to_string()),
                after: Some("formatDate(d: Date, locale: string): string".to_string()),
                description: "Added required 'locale' parameter".to_string(),
            }],
            breaking_behavioral_changes: vec![],
        }];

        let report = make_report(changes, vec![]);
        let empty_cache = HashMap::new();
        let rules = generate_rules(&report, "*.ts", &empty_cache, &RenamePatterns::empty());
        let guidance = generate_fix_guidance(&report, &rules, "*.ts");

        assert_eq!(guidance.fixes.len(), 1);
        let fix = &guidance.fixes[0];
        assert!(matches!(fix.strategy, FixStrategy::UpdateSignature));
        assert!(matches!(fix.confidence, FixConfidence::High));
        assert!(fix.fix_description.contains("Old signature:"));
        assert!(fix.fix_description.contains("New signature:"));
        assert_eq!(fix.before.as_deref(), Some("formatDate(d: Date): string"));
        assert_eq!(
            fix.after.as_deref(),
            Some("formatDate(d: Date, locale: string): string")
        );
    }

    #[test]
    fn test_fix_guidance_behavioral_is_llm_source() {
        let changes = vec![FileChanges {
            file: PathBuf::from("src/auth.ts"),
            status: FileStatus::Modified,
            renamed_from: None,
            breaking_api_changes: vec![],
            breaking_behavioral_changes: vec![BehavioralChange {
                symbol: "validateToken".to_string(),
                kind: BehavioralChangeKind::Function,
                category: None,
                description: "Now throws on expired tokens instead of returning null".to_string(),
                source_file: Some("src/auth.ts".to_string()),
            }],
        }];

        let report = make_report(changes, vec![]);
        let empty_cache = HashMap::new();
        let rules = generate_rules(&report, "*.ts", &empty_cache, &RenamePatterns::empty());
        let guidance = generate_fix_guidance(&report, &rules, "*.ts");

        assert_eq!(guidance.fixes.len(), 1);
        let fix = &guidance.fixes[0];
        assert!(matches!(fix.strategy, FixStrategy::ManualReview));
        assert!(matches!(fix.confidence, FixConfidence::Medium));
        assert!(matches!(fix.source, FixSource::Llm));
        assert!(fix.fix_description.contains("AI-generated"));
        assert!(fix.fix_description.contains("throws on expired tokens"));
    }

    #[test]
    fn test_fix_guidance_manifest_cjs_to_esm() {
        let manifest = vec![ManifestChange {
            field: "type".to_string(),
            change_type: ManifestChangeType::ModuleSystemChanged,
            before: Some("commonjs".to_string()),
            after: Some("module".to_string()),
            description: "CJS to ESM migration".to_string(),
            is_breaking: true,
        }];

        let report = make_report(vec![], manifest);
        let empty_cache = HashMap::new();
        let rules = generate_rules(&report, "*.ts", &empty_cache, &RenamePatterns::empty());
        let guidance = generate_fix_guidance(&report, &rules, "*.ts");

        assert_eq!(guidance.fixes.len(), 1);
        let fix = &guidance.fixes[0];
        assert!(matches!(fix.strategy, FixStrategy::UpdateImport));
        assert!(matches!(fix.confidence, FixConfidence::High));
        assert!(fix.fix_description.contains("require()"));
        assert!(fix.fix_description.contains("import"));
        assert_eq!(fix.replacement.as_deref(), Some("import"));
    }

    #[test]
    fn test_fix_guidance_summary_counts() {
        let changes = vec![FileChanges {
            file: PathBuf::from("src/lib.d.ts"),
            status: FileStatus::Modified,
            renamed_from: None,
            breaking_api_changes: vec![
                ApiChange {
                    symbol: "Chip".to_string(),
                    kind: ApiChangeKind::Class,
                    change: ApiChangeType::Renamed,
                    before: Some("Chip".to_string()),
                    after: Some("Label".to_string()),
                    description: "Renamed".to_string(),
                },
                ApiChange {
                    symbol: "oldFn".to_string(),
                    kind: ApiChangeKind::Function,
                    change: ApiChangeType::Removed,
                    before: None,
                    after: None,
                    description: "Removed".to_string(),
                },
            ],
            breaking_behavioral_changes: vec![BehavioralChange {
                symbol: "process".to_string(),
                kind: BehavioralChangeKind::Function,
                category: None,
                description: "Changed behavior".to_string(),
                source_file: Some("src/lib.ts".to_string()),
            }],
        }];

        let report = make_report(changes, vec![]);
        let empty_cache = HashMap::new();
        let rules = generate_rules(&report, "*.ts", &empty_cache, &RenamePatterns::empty());
        let guidance = generate_fix_guidance(&report, &rules, "*.ts");

        assert_eq!(guidance.summary.total_fixes, 3);
        // Rename=Exact (auto), Removed=Low/Manual, Behavioral=Medium/LLM
        assert_eq!(guidance.summary.auto_fixable, 1); // only Rename
        assert_eq!(guidance.summary.manual_only, 1); // Removed
        assert_eq!(guidance.summary.needs_review, 1); // Behavioral
    }

    #[test]
    fn test_fix_guidance_yaml_roundtrip() {
        let changes = vec![FileChanges {
            file: PathBuf::from("src/index.d.ts"),
            status: FileStatus::Modified,
            renamed_from: None,
            breaking_api_changes: vec![
                ApiChange {
                    symbol: "Foo".to_string(),
                    kind: ApiChangeKind::Class,
                    change: ApiChangeType::Renamed,
                    before: Some("Foo".to_string()),
                    after: Some("Bar".to_string()),
                    description: "Renamed Foo to Bar".to_string(),
                },
                ApiChange {
                    symbol: "baz".to_string(),
                    kind: ApiChangeKind::Function,
                    change: ApiChangeType::SignatureChanged,
                    before: Some("baz(): void".to_string()),
                    after: Some("baz(x: number): void".to_string()),
                    description: "Added required param".to_string(),
                },
            ],
            breaking_behavioral_changes: vec![],
        }];

        let manifest = vec![ManifestChange {
            field: "type".to_string(),
            change_type: ManifestChangeType::ModuleSystemChanged,
            before: Some("commonjs".to_string()),
            after: Some("module".to_string()),
            description: "CJS to ESM".to_string(),
            is_breaking: true,
        }];

        let report = make_report(changes, manifest);
        let empty_cache = HashMap::new();
        let rules = generate_rules(
            &report,
            "*.{ts,tsx}",
            &empty_cache,
            &RenamePatterns::empty(),
        );
        let guidance = generate_fix_guidance(&report, &rules, "*.{ts,tsx}");

        let yaml = serde_yaml::to_string(&guidance).unwrap();
        assert!(yaml.contains("strategy"));
        assert!(yaml.contains("confidence"));
        assert!(yaml.contains("fix_description"));
        assert!(yaml.contains("search_pattern"));
        assert!(yaml.contains("replacement"));
        assert!(yaml.contains("rename"));
        assert!(yaml.contains("update_signature"));
        assert!(yaml.contains("update_import"));
        assert!(yaml.contains("auto_fixable"));
        assert!(yaml.contains("needs_review"));
        assert!(yaml.contains("manual_only"));
    }

    // ── Frontend provider tests ─────────────────────────────────────

    #[test]
    fn test_frontend_provider_class_rename_generates_or_condition() {
        let changes = vec![FileChanges {
            file: PathBuf::from("src/components/Chip.d.ts"),
            status: FileStatus::Modified,
            renamed_from: None,
            breaking_api_changes: vec![ApiChange {
                symbol: "Chip".to_string(),
                kind: ApiChangeKind::Class,
                change: ApiChangeType::Renamed,
                before: Some("Chip".to_string()),
                after: Some("Label".to_string()),
                description: "Chip renamed to Label".to_string(),
            }],
            breaking_behavioral_changes: vec![],
        }];

        let report = make_report(changes, vec![]);
        let empty_cache = HashMap::new();
        let rules = generate_rules(&report, "*.ts", &empty_cache, &RenamePatterns::empty());

        assert_eq!(rules.len(), 1);
        let yaml = serde_yaml::to_string(&rules[0]).unwrap();
        // Should have an or: condition with JSX_COMPONENT and IMPORT
        assert!(yaml.contains("frontend.referenced"));
        assert!(yaml.contains("JSX_COMPONENT"));
        assert!(yaml.contains("IMPORT"));
        assert!(yaml.contains("^Chip$")); // matches old name
        assert!(yaml.contains("has-codemod=true"));
    }

    #[test]
    fn test_frontend_provider_prop_removed_scoped_to_component() {
        let changes = vec![FileChanges {
            file: PathBuf::from("src/components/Card.d.ts"),
            status: FileStatus::Modified,
            renamed_from: None,
            breaking_api_changes: vec![ApiChange {
                symbol: "Card.isFlat".to_string(),
                kind: ApiChangeKind::Property,
                change: ApiChangeType::Removed,
                before: None,
                after: None,
                description: "Card.isFlat prop removed".to_string(),
            }],
            breaking_behavioral_changes: vec![],
        }];

        let report = make_report(changes, vec![]);
        let empty_cache = HashMap::new();
        let rules = generate_rules(&report, "*.ts", &empty_cache, &RenamePatterns::empty());

        assert_eq!(rules.len(), 1);
        let yaml = serde_yaml::to_string(&rules[0]).unwrap();
        // Should use JSX_PROP location with component filter
        assert!(yaml.contains("JSX_PROP"));
        assert!(yaml.contains("^isFlat$"));
        assert!(yaml.contains("^Card$")); // component filter
    }

    #[test]
    fn test_frontend_provider_function_uses_function_call() {
        let changes = vec![FileChanges {
            file: PathBuf::from("src/utils.d.ts"),
            status: FileStatus::Modified,
            renamed_from: None,
            breaking_api_changes: vec![ApiChange {
                symbol: "createUser".to_string(),
                kind: ApiChangeKind::Function,
                change: ApiChangeType::Removed,
                before: None,
                after: None,
                description: "createUser removed".to_string(),
            }],
            breaking_behavioral_changes: vec![],
        }];

        let report = make_report(changes, vec![]);
        let empty_cache = HashMap::new();
        let rules = generate_rules(&report, "*.ts", &empty_cache, &RenamePatterns::empty());

        assert_eq!(rules.len(), 1);
        let yaml = serde_yaml::to_string(&rules[0]).unwrap();
        assert!(yaml.contains("FUNCTION_CALL"));
        assert!(yaml.contains("^createUser$"));
    }

    #[test]
    fn test_frontend_provider_type_alias_uses_type_reference() {
        let changes = vec![FileChanges {
            file: PathBuf::from("src/types.d.ts"),
            status: FileStatus::Modified,
            renamed_from: None,
            breaking_api_changes: vec![ApiChange {
                symbol: "UserRole".to_string(),
                kind: ApiChangeKind::TypeAlias,
                change: ApiChangeType::Removed,
                before: None,
                after: None,
                description: "UserRole type removed".to_string(),
            }],
            breaking_behavioral_changes: vec![],
        }];

        let report = make_report(changes, vec![]);
        let empty_cache = HashMap::new();
        let rules = generate_rules(&report, "*.ts", &empty_cache, &RenamePatterns::empty());

        assert_eq!(rules.len(), 1);
        let yaml = serde_yaml::to_string(&rules[0]).unwrap();
        assert!(yaml.contains("TYPE_REFERENCE"));
        assert!(yaml.contains("^UserRole$"));
    }

    #[test]
    fn test_frontend_provider_constant_uses_import() {
        let changes = vec![FileChanges {
            file: PathBuf::from("src/config.d.ts"),
            status: FileStatus::Modified,
            renamed_from: None,
            breaking_api_changes: vec![ApiChange {
                symbol: "DEFAULT_TIMEOUT".to_string(),
                kind: ApiChangeKind::Constant,
                change: ApiChangeType::Removed,
                before: None,
                after: None,
                description: "DEFAULT_TIMEOUT removed".to_string(),
            }],
            breaking_behavioral_changes: vec![],
        }];

        let report = make_report(changes, vec![]);
        let empty_cache = HashMap::new();
        let rules = generate_rules(&report, "*.ts", &empty_cache, &RenamePatterns::empty());

        assert_eq!(rules.len(), 1);
        let yaml = serde_yaml::to_string(&rules[0]).unwrap();
        assert!(yaml.contains("IMPORT"));
        assert!(yaml.contains("^DEFAULT_TIMEOUT$"));
    }
}
