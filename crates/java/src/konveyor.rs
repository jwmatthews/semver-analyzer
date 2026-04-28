//! Java Konveyor rule generator.
//!
//! Converts an `AnalysisReport<Java>` into Konveyor YAML rules using
//! `java.referenced` conditions for AST-level matching.
//!
//! Two generators:
//! - `generate_rules()` — TD rules from structural API diff
//! - `generate_sd_rules()` — SD rules from source-level behavioral analysis

use crate::language::Java;
use crate::sd_types::{JavaSdPipelineResult, JavaSourceCategory, JavaSourceChange};
use semver_analyzer_core::AnalysisReport;
use semver_analyzer_konveyor_core::{
    FixStrategyEntry, JavaDependencyFields, JavaReferencedFields, KonveyorCondition, KonveyorLink,
    KonveyorRule, KonveyorRuleset,
};
use std::collections::HashMap;

// ── Configuration ───────────────────────────────────────────────────────

/// Configuration for Java Konveyor rule generation.
///
/// Parameterizes the rule generator for different Java projects
/// (Spring Boot, Quarkus, Jakarta EE, etc.).
#[derive(Debug, Clone)]
pub struct JavaKonveyorConfig {
    /// Project name (e.g., "spring-boot"). Used in ruleset metadata.
    pub project_name: String,
    /// Rule ID prefix (e.g., "sb4"). Used in rule IDs.
    pub rule_id_prefix: String,
    /// Migration guide URL (optional).
    pub migration_guide_url: Option<String>,
    /// Migration guide title (optional).
    pub migration_guide_title: Option<String>,
}

impl Default for JavaKonveyorConfig {
    fn default() -> Self {
        Self {
            project_name: "java-library".into(),
            rule_id_prefix: "java".into(),
            migration_guide_url: None,
            migration_guide_title: None,
        }
    }
}

impl JavaKonveyorConfig {
    /// Create a config from CLI args.
    pub fn from_args(
        project_name: Option<&str>,
        rule_prefix: Option<&str>,
        migration_guide_url: Option<&str>,
    ) -> Self {
        let project = project_name.unwrap_or("java-library");
        let prefix = rule_prefix.unwrap_or_else(|| {
            // Derive prefix from project name: "spring-boot" → "sb"
            project
                .split('-')
                .filter_map(|w| w.chars().next())
                .collect::<String>()
                .as_str()
                .to_string()
                .leak() // Safe: called once per CLI invocation
        });
        Self {
            project_name: project.to_string(),
            rule_id_prefix: prefix.to_string(),
            migration_guide_url: migration_guide_url.map(|s| s.to_string()),
            migration_guide_title: migration_guide_url
                .map(|_| format!("{} Migration Guide", project)),
        }
    }
}

// ── Ruleset ─────────────────────────────────────────────────────────────

/// Generate a ruleset metadata file.
pub fn ruleset(from: &str, to: &str) -> KonveyorRuleset {
    ruleset_with_config(from, to, &JavaKonveyorConfig::default())
}

/// Generate a ruleset with custom config.
pub fn ruleset_with_config(from: &str, to: &str, config: &JavaKonveyorConfig) -> KonveyorRuleset {
    KonveyorRuleset {
        name: format!("{}-{}-to-{}", config.project_name, from, to),
        description: format!(
            "Auto-generated migration rules for {} {} to {}",
            config.project_name, from, to
        ),
        labels: vec!["source=semver-analyzer".into(), "language=java".into()],
    }
}

// ── TD rule generation ──────────────────────────────────────────────────

/// Generate TD rules from a Java analysis report.
pub fn generate_rules(report: &AnalysisReport<Java>) -> Vec<KonveyorRule> {
    generate_rules_with_config(report, &JavaKonveyorConfig::default())
}

/// Generate TD rules with custom config.
pub fn generate_rules_with_config(
    report: &AnalysisReport<Java>,
    config: &JavaKonveyorConfig,
) -> Vec<KonveyorRule> {
    let mut rules = Vec::new();
    let mut id_counts: HashMap<String, usize> = HashMap::new();

    let mut relocations: Vec<(&str, &str, &str)> = Vec::new();

    for fc in &report.changes {
        for ac in &fc.breaking_api_changes {
            match ac.change {
                semver_analyzer_core::ApiChangeType::Renamed => {
                    if let (Some(before), Some(after)) = (&ac.before, &ac.after) {
                        let before_class = before.rsplit('.').next().unwrap_or(before);
                        let after_class = after.rsplit('.').next().unwrap_or(after);
                        if before_class == after_class && before != after {
                            relocations.push((&ac.symbol, before, after));
                        } else if before_class != after_class {
                            rules.push(make_rename_rule(
                                &ac.symbol,
                                before,
                                after,
                                &ac.description,
                                config,
                                &mut id_counts,
                            ));
                        }
                    }
                }
                semver_analyzer_core::ApiChangeType::Removed => {
                    if let Some(ref mt) = ac.migration_target {
                        rules.push(make_removal_with_target_rule(
                            &ac.symbol,
                            &mt.removed_qualified_name,
                            &mt.replacement_symbol,
                            &mt.replacement_qualified_name,
                            &ac.description,
                            config,
                            &mut id_counts,
                        ));
                    } else {
                        let qname = ac.before.as_deref().unwrap_or(&ac.symbol);
                        rules.push(make_removal_rule(
                            &ac.symbol,
                            qname,
                            &ac.description,
                            config,
                            &mut id_counts,
                        ));
                    }
                }
                semver_analyzer_core::ApiChangeType::TypeChanged => {
                    if let (Some(before), Some(after)) = (&ac.before, &ac.after) {
                        rules.push(make_type_changed_rule(
                            &ac.symbol,
                            &ac.qualified_name,
                            before,
                            after,
                            &ac.description,
                            config,
                            &mut id_counts,
                        ));
                    }
                }
                semver_analyzer_core::ApiChangeType::SignatureChanged => {
                    if let (Some(before), Some(after)) = (&ac.before, &ac.after) {
                        rules.push(make_signature_changed_rule(
                            &ac.symbol,
                            &ac.qualified_name,
                            before,
                            after,
                            &ac.description,
                            config,
                            &mut id_counts,
                        ));
                    }
                }
                semver_analyzer_core::ApiChangeType::VisibilityChanged => {
                    if let (Some(before), Some(after)) = (&ac.before, &ac.after) {
                        rules.push(make_visibility_changed_rule(
                            &ac.symbol,
                            &ac.qualified_name,
                            before,
                            after,
                            &ac.description,
                            config,
                            &mut id_counts,
                        ));
                    }
                }
            }
        }
    }

    for &(name, old_qname, new_qname) in &relocations {
        rules.push(make_import_relocation_rule(
            name,
            old_qname,
            new_qname,
            config,
            &mut id_counts,
        ));
    }

    for mc in &report.manifest_changes {
        if mc.is_breaking {
            if let Some(ref before) = mc.before {
                rules.push(make_dependency_rule(
                    &mc.field,
                    before,
                    mc.after.as_deref(),
                    &mc.description,
                    config,
                    &mut id_counts,
                ));
            }
        }
    }

    rules
}

// ── SD rule generation ──────────────────────────────────────────────────

/// Generate SD rules from source-level diff results.
pub fn generate_sd_rules(
    sd: &JavaSdPipelineResult,
    config: &JavaKonveyorConfig,
) -> Vec<KonveyorRule> {
    let mut rules = Vec::new();
    let mut id_counts: HashMap<String, usize> = HashMap::new();

    // Generate rules from source-level changes
    for change in &sd.source_level_changes {
        if !change.is_breaking {
            continue;
        }
        if let Some(rule) = make_sd_rule(change, config, &mut id_counts) {
            rules.push(rule);
        }
    }

    // Generate rules from module changes
    for change in &sd.module_changes {
        if !change.is_breaking {
            continue;
        }
        if let Some(rule) = make_sd_rule(change, config, &mut id_counts) {
            rules.push(rule);
        }
    }

    rules
}

fn make_sd_rule(
    change: &JavaSourceChange,
    config: &JavaKonveyorConfig,
    id_counts: &mut HashMap<String, usize>,
) -> Option<KonveyorRule> {
    let (change_type_label, scope, effort) = match change.category {
        JavaSourceCategory::AnnotationRemoved => ("annotation-removed", "ANNOTATION", 3),
        JavaSourceCategory::AnnotationChanged => ("annotation-changed", "ANNOTATION", 2),
        JavaSourceCategory::SynchronizationRemoved => ("synchronization-removed", "METHOD_CALL", 3),
        JavaSourceCategory::ExceptionAdded => ("exception-added", "METHOD_CALL", 3),
        JavaSourceCategory::SerializationFieldRemoved => ("serialization-break", "TYPE", 5),
        JavaSourceCategory::SerializationFieldTypeChanged => ("serialization-break", "TYPE", 5),
        JavaSourceCategory::TransientChanged => ("serialization-break", "TYPE", 3),
        JavaSourceCategory::OverrideRemoved => ("override-removed", "TYPE", 3),
        JavaSourceCategory::ConstructorDependencyChanged => {
            ("constructor-changed", "CONSTRUCTOR_CALL", 3)
        }
        JavaSourceCategory::FinalAdded => ("final-added", "TYPE", 3),
        JavaSourceCategory::SealedChanged => ("sealed-changed", "TYPE", 3),
        JavaSourceCategory::InheritanceChanged => ("inheritance-changed", "TYPE", 5),
        JavaSourceCategory::NativeRemoved => ("native-removed", "METHOD_CALL", 5),
        JavaSourceCategory::DelegationChanged => ("delegation-changed", "METHOD_CALL", 3),
        JavaSourceCategory::ModuleExportRemoved => ("module-export-removed", "IMPORT", 5),
        // Non-breaking categories don't generate rules
        _ => return None,
    };

    let class_pattern = regex_escape(&change.class_name);
    let rule_id = unique_id(
        &format!(
            "{}-sd-{}-{}",
            config.rule_id_prefix,
            change_type_label,
            slugify(&change.class_name)
        ),
        id_counts,
    );

    let mut labels = vec![
        "source=semver-analyzer".into(),
        format!("change-type={}", change_type_label),
        "language=java".into(),
        "pipeline=sd".into(),
    ];

    if change.method.is_some() {
        labels.push("scope=method".into());
    }

    let links = config
        .migration_guide_url
        .as_ref()
        .map(|url| {
            vec![KonveyorLink {
                url: url.clone(),
                title: config
                    .migration_guide_title
                    .clone()
                    .unwrap_or_else(|| "Migration Guide".into()),
            }]
        })
        .unwrap_or_default();

    Some(KonveyorRule {
        rule_id,
        labels,
        effort,
        category: "mandatory".into(),
        description: change.description.clone(),
        message: build_sd_message(change),
        links,
        when: KonveyorCondition::JavaReferenced {
            referenced: JavaReferencedFields {
                pattern: class_pattern,
                scope: Some(scope.to_string()),
                ..Default::default()
            },
        },
        fix_strategy: build_sd_fix_strategy(change),
    })
}

fn build_sd_message(change: &JavaSourceChange) -> String {
    let mut msg = change.description.clone();

    if let (Some(old), Some(new)) = (&change.old_value, &change.new_value) {
        msg.push_str(&format!("\n\nBefore: `{}`\nAfter: `{}`", old, new));
    } else if let Some(old) = &change.old_value {
        msg.push_str(&format!("\n\nRemoved: `{}`", old));
    } else if let Some(new) = &change.new_value {
        msg.push_str(&format!("\n\nAdded: `{}`", new));
    }

    msg
}

fn build_sd_fix_strategy(change: &JavaSourceChange) -> Option<FixStrategyEntry> {
    match change.category {
        JavaSourceCategory::AnnotationRemoved | JavaSourceCategory::AnnotationChanged => {
            Some(FixStrategyEntry::new("ManualReview"))
        }
        JavaSourceCategory::FinalAdded | JavaSourceCategory::SealedChanged => {
            Some(FixStrategyEntry::new("ManualReview"))
        }
        JavaSourceCategory::InheritanceChanged => {
            if let (Some(old), Some(new)) = (&change.old_value, &change.new_value) {
                Some(FixStrategyEntry::with_from_to(
                    "UpdateSignature",
                    old,
                    new,
                ))
            } else {
                Some(FixStrategyEntry::new("ManualReview"))
            }
        }
        _ => None,
    }
}

// ── TD rule helpers ─────────────────────────────────────────────────────

fn make_import_relocation_rule(
    name: &str,
    old_qname: &str,
    new_qname: &str,
    config: &JavaKonveyorConfig,
    id_counts: &mut HashMap<String, usize>,
) -> KonveyorRule {
    let rule_id = unique_id(
        &format!("{}-import-{}", config.rule_id_prefix, slugify(name)),
        id_counts,
    );

    let old_pkg = old_qname
        .rsplit_once('.')
        .map(|(p, _)| p)
        .unwrap_or(old_qname);
    let new_pkg = new_qname
        .rsplit_once('.')
        .map(|(p, _)| p)
        .unwrap_or(new_qname);

    let links = config
        .migration_guide_url
        .as_ref()
        .map(|url| {
            vec![KonveyorLink {
                url: url.clone(),
                title: config
                    .migration_guide_title
                    .clone()
                    .unwrap_or_else(|| "Migration Guide".into()),
            }]
        })
        .unwrap_or_default();

    KonveyorRule {
        rule_id,
        labels: vec![
            "source=semver-analyzer".into(),
            "change-type=import-path-change".into(),
            "language=java".into(),
            "has-codemod=true".into(),
        ],
        effort: 1,
        category: "mandatory".into(),
        description: format!("`{}` moved from `{}` to `{}`", name, old_pkg, new_pkg),
        message: format!(
            "`{}` has been relocated.\n\n\
             Replace:\n  `import {}`\n\
             With:\n  `import {}`",
            name, old_qname, new_qname,
        ),
        links,
        when: KonveyorCondition::JavaReferenced {
            referenced: JavaReferencedFields {
                pattern: old_qname.to_string(),
                scope: Some("IMPORT".into()),
                ..Default::default()
            },
        },
        fix_strategy: Some(FixStrategyEntry::rename(old_qname, new_qname)),
    }
}

fn make_rename_rule(
    symbol: &str,
    old_name: &str,
    new_name: &str,
    description: &str,
    config: &JavaKonveyorConfig,
    id_counts: &mut HashMap<String, usize>,
) -> KonveyorRule {
    let rule_id = unique_id(
        &format!("{}-rename-{}", config.rule_id_prefix, slugify(symbol)),
        id_counts,
    );

    KonveyorRule {
        rule_id,
        labels: vec![
            "source=semver-analyzer".into(),
            "change-type=renamed".into(),
            "language=java".into(),
        ],
        effort: 3,
        category: "mandatory".into(),
        description: format!("`{}` renamed to `{}`", old_name, new_name),
        message: format!(
            "{}\n\nReplace `{}` with `{}`.",
            description, old_name, new_name,
        ),
        links: vec![],
        when: KonveyorCondition::Or {
            or: vec![
                KonveyorCondition::JavaReferenced {
                    referenced: JavaReferencedFields {
                        pattern: old_name.to_string(),
                        scope: Some("IMPORT".into()),
                        ..Default::default()
                    },
                },
                KonveyorCondition::JavaReferenced {
                    referenced: JavaReferencedFields {
                        pattern: old_name.to_string(),
                        scope: Some("TYPE".into()),
                        ..Default::default()
                    },
                },
            ],
        },
        fix_strategy: Some(FixStrategyEntry::rename(old_name, new_name)),
    }
}

fn make_removal_rule(
    symbol: &str,
    qualified_name: &str,
    description: &str,
    config: &JavaKonveyorConfig,
    id_counts: &mut HashMap<String, usize>,
) -> KonveyorRule {
    let rule_id = unique_id(
        &format!("{}-removed-{}", config.rule_id_prefix, slugify(symbol)),
        id_counts,
    );

    KonveyorRule {
        rule_id,
        labels: vec![
            "source=semver-analyzer".into(),
            "change-type=removed".into(),
            "language=java".into(),
        ],
        effort: 5,
        category: "mandatory".into(),
        description: format!("`{}` has been removed", symbol),
        message: format!(
            "{}\n\nThis class has been removed with no direct replacement.",
            description
        ),
        links: vec![],
        when: KonveyorCondition::JavaReferenced {
            referenced: JavaReferencedFields {
                pattern: qualified_name.to_string(),
                scope: Some("IMPORT".into()),
                ..Default::default()
            },
        },
        fix_strategy: None,
    }
}

fn make_removal_with_target_rule(
    symbol: &str,
    old_qname: &str,
    new_symbol: &str,
    new_qname: &str,
    description: &str,
    config: &JavaKonveyorConfig,
    id_counts: &mut HashMap<String, usize>,
) -> KonveyorRule {
    let rule_id = unique_id(
        &format!("{}-migrate-{}", config.rule_id_prefix, slugify(symbol)),
        id_counts,
    );

    KonveyorRule {
        rule_id,
        labels: vec![
            "source=semver-analyzer".into(),
            "change-type=removed".into(),
            "has-codemod=true".into(),
            "language=java".into(),
        ],
        effort: 3,
        category: "mandatory".into(),
        description: format!("`{}` removed -- migrate to `{}`", symbol, new_symbol),
        message: format!(
            "{}\n\nReplace:\n  `import {}`\nWith:\n  `import {}`",
            description, old_qname, new_qname,
        ),
        links: vec![],
        when: KonveyorCondition::JavaReferenced {
            referenced: JavaReferencedFields {
                pattern: old_qname.to_string(),
                scope: Some("IMPORT".into()),
                ..Default::default()
            },
        },
        fix_strategy: Some(FixStrategyEntry::rename(old_qname, new_qname)),
    }
}

fn make_type_changed_rule(
    symbol: &str,
    qualified_name: &str,
    before: &str,
    after: &str,
    description: &str,
    config: &JavaKonveyorConfig,
    id_counts: &mut HashMap<String, usize>,
) -> KonveyorRule {
    let rule_id = unique_id(
        &format!(
            "{}-type-changed-{}",
            config.rule_id_prefix,
            slugify(symbol)
        ),
        id_counts,
    );

    KonveyorRule {
        rule_id,
        labels: vec![
            "source=semver-analyzer".into(),
            "change-type=type-changed".into(),
            "language=java".into(),
        ],
        effort: 3,
        category: "mandatory".into(),
        description: format!("Type of `{}` changed: `{}` → `{}`", symbol, before, after),
        message: format!(
            "{}\n\nType changed from `{}` to `{}`.",
            description, before, after
        ),
        links: vec![],
        when: KonveyorCondition::JavaReferenced {
            referenced: JavaReferencedFields {
                pattern: regex_escape(qualified_name),
                scope: Some("TYPE".into()),
                ..Default::default()
            },
        },
        fix_strategy: Some(FixStrategyEntry::with_from_to("UpdateType", before, after)),
    }
}

fn make_signature_changed_rule(
    symbol: &str,
    qualified_name: &str,
    before: &str,
    after: &str,
    description: &str,
    config: &JavaKonveyorConfig,
    id_counts: &mut HashMap<String, usize>,
) -> KonveyorRule {
    let rule_id = unique_id(
        &format!(
            "{}-sig-changed-{}",
            config.rule_id_prefix,
            slugify(symbol)
        ),
        id_counts,
    );

    KonveyorRule {
        rule_id,
        labels: vec![
            "source=semver-analyzer".into(),
            "change-type=signature-changed".into(),
            "language=java".into(),
        ],
        effort: 3,
        category: "mandatory".into(),
        description: format!("Signature of `{}` changed", symbol),
        message: format!(
            "{}\n\nBefore: `{}`\nAfter: `{}`",
            description, before, after
        ),
        links: vec![],
        when: KonveyorCondition::JavaReferenced {
            referenced: JavaReferencedFields {
                pattern: regex_escape(qualified_name),
                scope: Some("METHOD_CALL".into()),
                ..Default::default()
            },
        },
        fix_strategy: Some(FixStrategyEntry::with_from_to(
            "UpdateSignature",
            before,
            after,
        )),
    }
}

fn make_visibility_changed_rule(
    symbol: &str,
    qualified_name: &str,
    before: &str,
    after: &str,
    description: &str,
    config: &JavaKonveyorConfig,
    id_counts: &mut HashMap<String, usize>,
) -> KonveyorRule {
    let rule_id = unique_id(
        &format!(
            "{}-visibility-{}",
            config.rule_id_prefix,
            slugify(symbol)
        ),
        id_counts,
    );

    KonveyorRule {
        rule_id,
        labels: vec![
            "source=semver-analyzer".into(),
            "change-type=visibility-changed".into(),
            "language=java".into(),
        ],
        effort: 3,
        category: "mandatory".into(),
        description: format!(
            "Visibility of `{}` changed: {} → {}",
            symbol, before, after
        ),
        message: format!(
            "{}\n\nVisibility narrowed from `{}` to `{}`.",
            description, before, after
        ),
        links: vec![],
        when: KonveyorCondition::JavaReferenced {
            referenced: JavaReferencedFields {
                pattern: regex_escape(qualified_name),
                scope: Some("TYPE".into()),
                ..Default::default()
            },
        },
        fix_strategy: Some(FixStrategyEntry::new("ManualReview")),
    }
}

fn make_dependency_rule(
    field: &str,
    before: &str,
    after: Option<&str>,
    description: &str,
    config: &JavaKonveyorConfig,
    id_counts: &mut HashMap<String, usize>,
) -> KonveyorRule {
    let dep_name = field.strip_prefix("dependency:").unwrap_or(field);
    let rule_id = unique_id(
        &format!("{}-dep-{}", config.rule_id_prefix, slugify(dep_name)),
        id_counts,
    );

    let message = if let Some(new) = after {
        format!("{}\n\nReplace `{}` with `{}`.", description, before, new)
    } else {
        format!("{}\n\nThis dependency has been removed.", description)
    };

    KonveyorRule {
        rule_id,
        labels: vec![
            "source=semver-analyzer".into(),
            "change-type=dependency-update".into(),
            "language=java".into(),
        ],
        effort: 1,
        category: "mandatory".into(),
        description: description.to_string(),
        message,
        links: vec![],
        when: KonveyorCondition::JavaDependency {
            dependency: JavaDependencyFields {
                name: Some(dep_name.to_string()),
                nameregex: None,
                upperbound: None,
                lowerbound: None,
            },
        },
        fix_strategy: None,
    }
}

// ── Helpers ─────────────────────────────────────────────────────────────

fn unique_id(base: &str, counts: &mut HashMap<String, usize>) -> String {
    let count = counts.entry(base.to_string()).or_insert(0);
    *count += 1;
    if *count == 1 {
        base.to_string()
    } else {
        format!("{}-{}", base, count)
    }
}

fn slugify(s: &str) -> String {
    s.chars()
        .map(|c| match c {
            '.' | '/' | ':' | '@' | ' ' => '-',
            c if c.is_alphanumeric() || c == '-' || c == '_' => c,
            _ => '-',
        })
        .collect::<String>()
        .to_lowercase()
}

fn regex_escape(s: &str) -> String {
    s.replace('.', "\\.")
}
