//! Baseline integration tests for Java Konveyor rule generation.
//!
//! Constructs analysis reports with known changes, generates rules,
//! and captures the output as insta snapshots.

mod helpers;

use semver_analyzer_core::*;
use semver_analyzer_java::extensions::JavaAnalysisExtensions;
use semver_analyzer_java::konveyor::{self, JavaKonveyorConfig};
use semver_analyzer_java::sd_types::*;
use semver_analyzer_java::Java;
use serde::Serialize;
use std::collections::HashMap;

// ── Normalized Konveyor rule for snapshotting ────────────────────────

#[derive(Debug, Serialize)]
struct NormalizedRule {
    rule_id: String,
    labels: Vec<String>,
    effort: u32,
    category: String,
    description: String,
    has_fix_strategy: bool,
}

fn normalize_rules(
    rules: &[semver_analyzer_konveyor_core::KonveyorRule],
) -> Vec<NormalizedRule> {
    rules
        .iter()
        .map(|r| NormalizedRule {
            rule_id: r.rule_id.clone(),
            labels: r.labels.clone(),
            effort: r.effort,
            category: r.category.clone(),
            description: r.description.clone(),
            has_fix_strategy: r.fix_strategy.is_some(),
        })
        .collect()
}

// ── Report construction helper ──────────────────────────────────────

fn make_report(
    changes: Vec<FileChanges<Java>>,
    manifest_changes: Vec<ManifestChange<Java>>,
) -> AnalysisReport<Java> {
    AnalysisReport {
        repository: std::path::PathBuf::from("/test/repo"),
        comparison: Comparison {
            from_ref: "v1.0.0".into(),
            to_ref: "v2.0.0".into(),
            from_sha: "abc123".into(),
            to_sha: "def456".into(),
            commit_count: 10,
            analysis_timestamp: "2025-01-01T00:00:00Z".into(),
        },
        summary: Summary {
            total_breaking_changes: changes
                .iter()
                .map(|fc| fc.breaking_api_changes.len())
                .sum(),
            breaking_api_changes: changes
                .iter()
                .map(|fc| fc.breaking_api_changes.len())
                .sum(),
            breaking_behavioral_changes: 0,
            files_with_breaking_changes: changes.len(),
        },
        changes,
        manifest_changes,
        added_files: Vec::new(),
        packages: Vec::new(),
        member_renames: HashMap::new(),
        inferred_rename_patterns: None,
        extensions: JavaAnalysisExtensions::default(),
        metadata: AnalysisMetadata {
            call_graph_analysis: String::new(),
            tool_version: "test".into(),
            llm_usage: None,
        },
    }
}

fn make_file_changes(api_changes: Vec<ApiChange>) -> FileChanges<Java> {
    FileChanges {
        file: std::path::PathBuf::from("src/main/java/com/example/Service.java"),
        status: FileStatus::Modified,
        renamed_from: None,
        breaking_api_changes: api_changes,
        breaking_behavioral_changes: Vec::new(),
        container_changes: Vec::new(),
    }
}

// ── TD rule tests ───────────────────────────────────────────────────

#[test]
fn konveyor_td_class_renamed() {
    let report = make_report(
        vec![make_file_changes(vec![ApiChange {
            symbol: "OldService".into(),
            qualified_name: "com.example.OldService".into(),
            kind: ApiChangeKind::Class,
            change: ApiChangeType::Renamed,
            before: Some("com.example.OldService".into()),
            after: Some("com.example.NewService".into()),
            description: "Class renamed from OldService to NewService".into(),
            migration_target: None,
            removal_disposition: None,
        }])],
        vec![],
    );

    let config = JavaKonveyorConfig::default();
    let rules = konveyor::generate_rules_with_config(&report, &config);
    insta::assert_yaml_snapshot!(normalize_rules(&rules));
}

#[test]
fn konveyor_td_class_removed() {
    let report = make_report(
        vec![make_file_changes(vec![ApiChange {
            symbol: "DeprecatedHelper".into(),
            qualified_name: "com.example.DeprecatedHelper".into(),
            kind: ApiChangeKind::Class,
            change: ApiChangeType::Removed,
            before: Some("com.example.DeprecatedHelper".into()),
            after: None,
            description: "Class removed with no replacement".into(),
            migration_target: None,
            removal_disposition: None,
        }])],
        vec![],
    );

    let config = JavaKonveyorConfig::default();
    let rules = konveyor::generate_rules_with_config(&report, &config);
    insta::assert_yaml_snapshot!(normalize_rules(&rules));
}

#[test]
fn konveyor_td_class_relocated() {
    let report = make_report(
        vec![make_file_changes(vec![ApiChange {
            symbol: "CacheManager".into(),
            qualified_name: "com.example.cache.CacheManager".into(),
            kind: ApiChangeKind::Class,
            change: ApiChangeType::Renamed,
            before: Some("com.example.cache.CacheManager".into()),
            after: Some("com.example.cache.auto.CacheManager".into()),
            description: "CacheManager relocated to cache.auto package".into(),
            migration_target: None,
            removal_disposition: None,
        }])],
        vec![],
    );

    let config = JavaKonveyorConfig::default();
    let rules = konveyor::generate_rules_with_config(&report, &config);
    insta::assert_yaml_snapshot!(normalize_rules(&rules));
}

#[test]
fn konveyor_td_type_changed() {
    let report = make_report(
        vec![make_file_changes(vec![ApiChange {
            symbol: "timeout".into(),
            qualified_name: "com.example.Config.timeout".into(),
            kind: ApiChangeKind::Property,
            change: ApiChangeType::TypeChanged,
            before: Some("int".into()),
            after: Some("Duration".into()),
            description: "Field type changed from int to Duration".into(),
            migration_target: None,
            removal_disposition: None,
        }])],
        vec![],
    );

    let config = JavaKonveyorConfig::default();
    let rules = konveyor::generate_rules_with_config(&report, &config);
    insta::assert_yaml_snapshot!(normalize_rules(&rules));
}

#[test]
fn konveyor_td_custom_config() {
    let report = make_report(
        vec![make_file_changes(vec![ApiChange {
            symbol: "OldService".into(),
            qualified_name: "com.example.OldService".into(),
            kind: ApiChangeKind::Class,
            change: ApiChangeType::Removed,
            before: Some("com.example.OldService".into()),
            after: None,
            description: "Class removed".into(),
            migration_target: None,
            removal_disposition: None,
        }])],
        vec![],
    );

    let config = JavaKonveyorConfig {
        project_name: "spring-boot".into(),
        rule_id_prefix: "sb4".into(),
        migration_guide_url: Some(
            "https://github.com/spring-projects/spring-boot/wiki/Migration-Guide".into(),
        ),
        migration_guide_title: Some("Spring Boot Migration Guide".into()),
    };
    let rules = konveyor::generate_rules_with_config(&report, &config);
    insta::assert_yaml_snapshot!(normalize_rules(&rules));
}

// ── SD rule tests ───────────────────────────────────────────────────

#[test]
fn konveyor_sd_annotation_removed() {
    let sd_result = JavaSdPipelineResult {
        source_level_changes: vec![JavaSourceChange {
            class_name: "com.example.Service".into(),
            category: JavaSourceCategory::AnnotationRemoved,
            description: "Annotation `@Bean` removed from `dataSource`".into(),
            old_value: Some("@Bean".into()),
            new_value: None,
            is_breaking: true,
            method: Some("dataSource".into()),
            dependency_chain: None,
        }],
        module_changes: vec![],
        ..Default::default()
    };

    let config = JavaKonveyorConfig::default();
    let rules = konveyor::generate_sd_rules(&sd_result, &config);
    insta::assert_yaml_snapshot!(normalize_rules(&rules));
}

#[test]
fn konveyor_sd_synchronized_removed() {
    let sd_result = JavaSdPipelineResult {
        source_level_changes: vec![JavaSourceChange {
            class_name: "com.example.Cache".into(),
            category: JavaSourceCategory::SynchronizationRemoved,
            description: "Method `update` is no longer synchronized".into(),
            old_value: Some("synchronized".into()),
            new_value: None,
            is_breaking: true,
            method: Some("update".into()),
            dependency_chain: None,
        }],
        module_changes: vec![],
        ..Default::default()
    };

    let config = JavaKonveyorConfig::default();
    let rules = konveyor::generate_sd_rules(&sd_result, &config);
    insta::assert_yaml_snapshot!(normalize_rules(&rules));
}

#[test]
fn konveyor_sd_module_export_removed() {
    let sd_result = JavaSdPipelineResult {
        source_level_changes: vec![],
        module_changes: vec![JavaSourceChange {
            class_name: "module-info".into(),
            category: JavaSourceCategory::ModuleExportRemoved,
            description: "Module directive removed: `exports com.example.internal`".into(),
            old_value: Some("exports com.example.internal".into()),
            new_value: None,
            is_breaking: true,
            method: None,
            dependency_chain: None,
        }],
        ..Default::default()
    };

    let config = JavaKonveyorConfig::default();
    let rules = konveyor::generate_sd_rules(&sd_result, &config);
    insta::assert_yaml_snapshot!(normalize_rules(&rules));
}

#[test]
fn konveyor_sd_non_breaking_skipped() {
    let sd_result = JavaSdPipelineResult {
        source_level_changes: vec![JavaSourceChange {
            class_name: "com.example.Service".into(),
            category: JavaSourceCategory::AnnotationAdded,
            description: "Annotation `@Deprecated` added to `Service`".into(),
            old_value: None,
            new_value: Some("@Deprecated".into()),
            is_breaking: false,
            method: None,
            dependency_chain: None,
        }],
        module_changes: vec![],
        ..Default::default()
    };

    let config = JavaKonveyorConfig::default();
    let rules = konveyor::generate_sd_rules(&sd_result, &config);
    // Non-breaking changes should not generate rules
    insta::assert_yaml_snapshot!(normalize_rules(&rules));
}
