//! Java Konveyor rule generator.
//!
//! Converts an `AnalysisReport<Java>` into Konveyor YAML rules using
//! `java.referenced` conditions for AST-level matching.

use crate::language::Java;
use semver_analyzer_core::AnalysisReport;
use semver_analyzer_konveyor_core::{
    FixStrategyEntry, JavaDependencyFields, JavaReferencedFields, KonveyorCondition, KonveyorLink,
    KonveyorRule, KonveyorRuleset,
};
use std::collections::HashMap;

/// Generate a ruleset metadata file for the Java migration rules.
pub fn ruleset(from: &str, to: &str) -> KonveyorRuleset {
    KonveyorRuleset {
        name: format!("spring-boot-{}-to-{}", from, to),
        description: format!(
            "Auto-generated migration rules for Spring Boot {} to {}",
            from, to
        ),
        labels: vec!["source=semver-analyzer".into(), "language=java".into()],
    }
}

/// Generate Konveyor rules from a Java analysis report.
pub fn generate_rules(report: &AnalysisReport<Java>) -> Vec<KonveyorRule> {
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
                            &mut id_counts,
                        ));
                    } else {
                        let qname = ac.before.as_deref().unwrap_or(&ac.symbol);
                        rules.push(make_removal_rule(
                            &ac.symbol,
                            qname,
                            &ac.description,
                            &mut id_counts,
                        ));
                    }
                }
                _ => {}
            }
        }
    }

    for &(name, old_qname, new_qname) in &relocations {
        rules.push(make_import_relocation_rule(
            name,
            old_qname,
            new_qname,
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
                    &mut id_counts,
                ));
            }
        }
    }

    rules
}

fn make_import_relocation_rule(
    name: &str,
    old_qname: &str,
    new_qname: &str,
    id_counts: &mut HashMap<String, usize>,
) -> KonveyorRule {
    let rule_id = unique_id(&format!("sb4-import-{}", slugify(name)), id_counts);

    let old_pkg = old_qname
        .rsplit_once('.')
        .map(|(p, _)| p)
        .unwrap_or(old_qname);
    let new_pkg = new_qname
        .rsplit_once('.')
        .map(|(p, _)| p)
        .unwrap_or(new_qname);

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
        links: vec![KonveyorLink {
            url: "https://github.com/spring-projects/spring-boot/wiki/Spring-Boot-4.0-Migration-Guide".into(),
            title: "Spring Boot 4.0 Migration Guide".into(),
        }],
        when: KonveyorCondition::JavaReferenced {
            referenced: JavaReferencedFields {
                pattern: old_qname.to_string(),
                location: Some("IMPORT".into()),
                annotated: None,
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
    id_counts: &mut HashMap<String, usize>,
) -> KonveyorRule {
    let rule_id = unique_id(&format!("sb4-rename-{}", slugify(symbol)), id_counts);

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
                        location: Some("IMPORT".into()),
                        annotated: None,
                    },
                },
                KonveyorCondition::JavaReferenced {
                    referenced: JavaReferencedFields {
                        pattern: old_name.to_string(),
                        location: Some("TYPE".into()),
                        annotated: None,
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
    id_counts: &mut HashMap<String, usize>,
) -> KonveyorRule {
    let rule_id = unique_id(&format!("sb4-removed-{}", slugify(symbol)), id_counts);

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
                location: Some("IMPORT".into()),
                annotated: None,
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
    id_counts: &mut HashMap<String, usize>,
) -> KonveyorRule {
    let rule_id = unique_id(&format!("sb4-migrate-{}", slugify(symbol)), id_counts);

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
                location: Some("IMPORT".into()),
                annotated: None,
            },
        },
        fix_strategy: Some(FixStrategyEntry::rename(old_qname, new_qname)),
    }
}

fn make_dependency_rule(
    field: &str,
    before: &str,
    after: Option<&str>,
    description: &str,
    id_counts: &mut HashMap<String, usize>,
) -> KonveyorRule {
    let dep_name = field.strip_prefix("dependency:").unwrap_or(field);
    let rule_id = unique_id(&format!("sb4-dep-{}", slugify(dep_name)), id_counts);

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
