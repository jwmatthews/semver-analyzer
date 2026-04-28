//! Java report builder.
//!
//! Converts raw `AnalysisResult<Java>` into a structured `AnalysisReport<Java>`
//! with file-level grouping, SHA resolution, and SD pipeline integration.

use crate::extensions::JavaAnalysisExtensions;
use crate::language::Java;
use crate::sd_types::JavaSourceCategory;
use crate::types::JavaReportData;
use semver_analyzer_core::{
    AnalysisMetadata, AnalysisReport, AnalysisResult, ApiChange, Comparison, FileChanges,
    FileStatus, Summary,
};
use std::collections::HashMap;
use std::path::{Path, PathBuf};
use std::process::Command;

/// Build the Java analysis report from raw results.
pub fn build_report(
    results: &AnalysisResult<Java>,
    repo: &Path,
    from_ref: &str,
    to_ref: &str,
) -> AnalysisReport<Java> {
    // Build a lookup table for fast symbol→file resolution
    let file_lookup = build_file_lookup(&results.old_surface, &results.new_surface);

    let mut file_map: HashMap<PathBuf, Vec<ApiChange>> = HashMap::new();

    for change in results.structural_changes.iter() {
        if !change.is_breaking {
            continue;
        }

        let file = file_lookup
            .get(&change.qualified_name)
            .cloned()
            .unwrap_or_else(|| PathBuf::from("unknown"));

        let api_change = ApiChange {
            symbol: change.symbol.clone(),
            qualified_name: change.qualified_name.clone(),
            kind: change.kind.into(),
            change: change.change_type.to_api_change_type(),
            before: change.before.clone(),
            after: change.after.clone(),
            description: change.description.clone(),
            migration_target: change.migration_target.clone(),
            removal_disposition: None,
        };

        file_map.entry(file).or_default().push(api_change);
    }

    let mut changes: Vec<FileChanges<Java>> = file_map
        .into_iter()
        .map(|(file, api_changes)| FileChanges {
            file,
            status: FileStatus::Modified,
            renamed_from: None,
            breaking_api_changes: api_changes,
            breaking_behavioral_changes: Vec::new(),
            container_changes: Vec::new(),
        })
        .collect();

    changes.sort_by(|a, b| a.file.cmp(&b.file));

    let breaking_api = results
        .structural_changes
        .iter()
        .filter(|c| c.is_breaking)
        .count();

    let breaking_behavioral = results
        .behavioral_changes
        .iter()
        .filter(|c| !c.is_internal_only.unwrap_or(false))
        .count();

    let files_with_breaking = changes.len();

    // Resolve git SHAs
    let from_sha = resolve_sha(repo, from_ref).unwrap_or_default();
    let to_sha = resolve_sha(repo, to_ref).unwrap_or_default();
    let commit_count = count_commits(repo, from_ref, to_ref).unwrap_or(0);
    let analysis_timestamp = {
        let output = Command::new("date").arg("-u").arg("+%Y-%m-%dT%H:%M:%SZ").output();
        output
            .ok()
            .map(|o| String::from_utf8_lossy(&o.stdout).trim().to_string())
            .unwrap_or_default()
    };

    // Build extensions with SD stats
    let extensions = build_report_extensions(&results.extensions);

    AnalysisReport {
        repository: repo.to_path_buf(),
        comparison: Comparison {
            from_ref: from_ref.to_string(),
            to_ref: to_ref.to_string(),
            from_sha,
            to_sha,
            commit_count,
            analysis_timestamp,
        },
        summary: Summary {
            total_breaking_changes: breaking_api + breaking_behavioral,
            breaking_api_changes: breaking_api,
            breaking_behavioral_changes: breaking_behavioral,
            files_with_breaking_changes: files_with_breaking,
        },
        changes,
        manifest_changes: results.manifest_changes.clone(),
        added_files: Vec::new(),
        packages: Vec::new(),
        member_renames: HashMap::new(),
        inferred_rename_patterns: results.inferred_rename_patterns.clone(),
        extensions,
        metadata: AnalysisMetadata {
            call_graph_analysis: String::new(),
            tool_version: env!("CARGO_PKG_VERSION").to_string(),
            llm_usage: None,
        },
    }
}

/// Build file lookup table from both API surfaces.
fn build_file_lookup(
    old_surface: &semver_analyzer_core::ApiSurface<crate::types::JavaSymbolData>,
    new_surface: &semver_analyzer_core::ApiSurface<crate::types::JavaSymbolData>,
) -> HashMap<String, PathBuf> {
    let mut lookup = HashMap::new();
    // New surface takes precedence (symbol may have moved)
    for surface in [old_surface, new_surface] {
        for sym in &surface.symbols {
            lookup.insert(sym.qualified_name.clone(), sym.file.clone());
            for member in &sym.members {
                lookup.insert(member.qualified_name.clone(), sym.file.clone());
            }
        }
    }
    lookup
}

/// Build report extensions from analysis extensions.
fn build_report_extensions(extensions: &JavaAnalysisExtensions) -> JavaAnalysisExtensions {
    let result = extensions.clone();

    // Ensure SD stats are available in the report
    if let Some(ref sd) = result.sd_result {
        let _report_data = JavaReportData {
            source_level_changes: sd.source_level_changes.len(),
            breaking_source_changes: sd
                .source_level_changes
                .iter()
                .filter(|c| c.is_breaking)
                .count(),
            annotation_changes: sd
                .source_level_changes
                .iter()
                .filter(|c| {
                    matches!(
                        c.category,
                        JavaSourceCategory::AnnotationRemoved
                            | JavaSourceCategory::AnnotationAdded
                            | JavaSourceCategory::AnnotationChanged
                    )
                })
                .count(),
            module_changes: sd.module_changes.len(),
            serialization_issues: sd
                .source_level_changes
                .iter()
                .filter(|c| {
                    matches!(
                        c.category,
                        JavaSourceCategory::SerializationFieldAdded
                            | JavaSourceCategory::SerializationFieldRemoved
                            | JavaSourceCategory::SerializationFieldTypeChanged
                            | JavaSourceCategory::TransientChanged
                    )
                })
                .count(),
        };
        // The report data is logged but not stored per-package yet
        // (Java doesn't have per-package TypeSummary like TypeScript).
        // The SD result itself is carried in the extensions.
    }

    result
}

// ── Git helpers ─────────────────────────────────────────────────────────

fn resolve_sha(repo: &Path, git_ref: &str) -> Option<String> {
    let output = Command::new("git")
        .args(["rev-parse", "--short", git_ref])
        .current_dir(repo)
        .output()
        .ok()?;

    if output.status.success() {
        Some(
            String::from_utf8_lossy(&output.stdout)
                .trim()
                .to_string(),
        )
    } else {
        None
    }
}

fn count_commits(repo: &Path, from_ref: &str, to_ref: &str) -> Option<usize> {
    let output = Command::new("git")
        .args([
            "rev-list",
            "--count",
            &format!("{}..{}", from_ref, to_ref),
        ])
        .current_dir(repo)
        .output()
        .ok()?;

    if output.status.success() {
        String::from_utf8_lossy(&output.stdout)
            .trim()
            .parse()
            .ok()
    } else {
        None
    }
}
