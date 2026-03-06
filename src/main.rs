mod cli;

use anyhow::{Context, Result};
use clap::Parser;
use std::path::Path;

use cli::{Cli, Command};
use semver_analyzer_core::{
    AnalysisMetadata, AnalysisReport, ApiSurface, Comparison, FileChanges, FileStatus,
    ManifestChange, StructuralChange, StructuralChangeType, Summary,
};
use semver_analyzer_ts::OxcExtractor;

#[tokio::main]
async fn main() -> Result<()> {
    let cli = Cli::parse();

    match cli.command {
        Command::Extract {
            repo,
            git_ref,
            output,
            build_command,
        } => {
            cmd_extract(&repo, &git_ref, output.as_deref(), build_command.as_deref())?;
        }

        Command::Diff { from, to, output } => {
            cmd_diff(&from, &to, output.as_deref())?;
        }

        Command::Analyze {
            repo,
            from,
            to,
            output,
            no_llm,
            llm_command,
            max_llm_cost,
            build_command,
        } => {
            cmd_analyze(
                &repo,
                &from,
                &to,
                output.as_deref(),
                no_llm,
                llm_command.as_deref(),
                max_llm_cost,
                build_command.as_deref(),
            )?;
        }

        Command::Serve => {
            eprintln!("MCP server not yet implemented");
        }
    }

    Ok(())
}

// ─── Extract command ─────────────────────────────────────────────────────

fn cmd_extract(
    repo: &Path,
    git_ref: &str,
    output: Option<&Path>,
    build_command: Option<&str>,
) -> Result<()> {
    eprintln!(
        "Extracting API surface from {} at ref {}",
        repo.display(),
        git_ref
    );

    let extractor = OxcExtractor::new();
    let surface = extractor
        .extract_at_ref(repo, git_ref, build_command)
        .context("Failed to extract API surface")?;

    eprintln!(
        "Extracted {} symbols from {} files",
        surface.symbols.len(),
        count_unique_files(&surface)
    );

    write_json_output(&surface, output)?;
    Ok(())
}

// ─── Diff command ────────────────────────────────────────────────────────

fn cmd_diff(from_path: &Path, to_path: &Path, output: Option<&Path>) -> Result<()> {
    eprintln!(
        "Diffing {} vs {}",
        from_path.display(),
        to_path.display()
    );

    let old_json = std::fs::read_to_string(from_path)
        .with_context(|| format!("Failed to read {}", from_path.display()))?;
    let new_json = std::fs::read_to_string(to_path)
        .with_context(|| format!("Failed to read {}", to_path.display()))?;

    let old: ApiSurface = serde_json::from_str(&old_json)
        .with_context(|| format!("Failed to parse {} as ApiSurface", from_path.display()))?;
    let new: ApiSurface = serde_json::from_str(&new_json)
        .with_context(|| format!("Failed to parse {} as ApiSurface", to_path.display()))?;

    let changes = semver_analyzer_core::diff::diff_surfaces(&old, &new);

    let breaking = changes.iter().filter(|c| c.is_breaking).count();
    let non_breaking = changes.len() - breaking;
    eprintln!(
        "Found {} changes ({} breaking, {} non-breaking)",
        changes.len(),
        breaking,
        non_breaking
    );

    write_json_output(&changes, output)?;
    Ok(())
}

// ─── Analyze command (full TD pipeline) ──────────────────────────────────

fn cmd_analyze(
    repo: &Path,
    from_ref: &str,
    to_ref: &str,
    output: Option<&Path>,
    _no_llm: bool,
    _llm_command: Option<&str>,
    _max_llm_cost: f64,
    build_command: Option<&str>,
) -> Result<()> {
    eprintln!(
        "Analyzing {} from {} to {}",
        repo.display(),
        from_ref,
        to_ref
    );

    // Step 1: Extract API surfaces for both refs
    let extractor = OxcExtractor::new();

    eprintln!("Extracting API surface at {} ...", from_ref);
    let old_surface = extractor
        .extract_at_ref(repo, from_ref, build_command)
        .with_context(|| format!("Failed to extract API surface at ref {}", from_ref))?;
    eprintln!("  {} symbols extracted", old_surface.symbols.len());

    eprintln!("Extracting API surface at {} ...", to_ref);
    let new_surface = extractor
        .extract_at_ref(repo, to_ref, build_command)
        .with_context(|| format!("Failed to extract API surface at ref {}", to_ref))?;
    eprintln!("  {} symbols extracted", new_surface.symbols.len());

    // Step 2: Structural diff
    eprintln!("Computing structural diff ...");
    let structural_changes = semver_analyzer_core::diff::diff_surfaces(&old_surface, &new_surface);
    let structural_breaking = structural_changes.iter().filter(|c| c.is_breaking).count();
    eprintln!(
        "  {} structural changes ({} breaking)",
        structural_changes.len(),
        structural_breaking
    );

    // Step 3: Package.json diff
    eprintln!("Comparing package.json ...");
    let manifest_changes = diff_package_json(repo, from_ref, to_ref);
    let manifest_breaking = manifest_changes.iter().filter(|c| c.is_breaking).count();
    if !manifest_changes.is_empty() {
        eprintln!(
            "  {} manifest changes ({} breaking)",
            manifest_changes.len(),
            manifest_breaking
        );
    }

    // Step 4: Build report
    let report = build_report(
        repo,
        from_ref,
        to_ref,
        structural_changes,
        manifest_changes,
    );

    let total_breaking = report.summary.total_breaking_changes;
    eprintln!();
    if total_breaking == 0 {
        eprintln!("No breaking changes detected.");
    } else {
        eprintln!(
            "BREAKING: {} total breaking change(s) detected.",
            total_breaking
        );
    }

    write_json_output(&report, output)?;
    Ok(())
}

// ─── Package.json diff helper ────────────────────────────────────────────

/// Try to diff package.json between two refs.
/// Falls back to empty if package.json doesn't exist at either ref.
fn diff_package_json(
    repo: &Path,
    from_ref: &str,
    to_ref: &str,
) -> Vec<ManifestChange> {
    let old_json = read_git_file(repo, from_ref, "package.json");
    let new_json = read_git_file(repo, to_ref, "package.json");

    match (old_json, new_json) {
        (Some(old_str), Some(new_str)) => {
            let old: serde_json::Value = match serde_json::from_str(&old_str) {
                Ok(v) => v,
                Err(e) => {
                    eprintln!("  Warning: could not parse package.json at {}: {}", from_ref, e);
                    return Vec::new();
                }
            };
            let new: serde_json::Value = match serde_json::from_str(&new_str) {
                Ok(v) => v,
                Err(e) => {
                    eprintln!("  Warning: could not parse package.json at {}: {}", to_ref, e);
                    return Vec::new();
                }
            };
            semver_analyzer_ts::manifest::diff_manifests(&old, &new)
        }
        _ => {
            eprintln!("  package.json not found at one or both refs, skipping manifest diff");
            Vec::new()
        }
    }
}

/// Read a file at a specific git ref via `git show`.
fn read_git_file(repo: &Path, git_ref: &str, file_path: &str) -> Option<String> {
    let output = std::process::Command::new("git")
        .args(["show", &format!("{}:{}", git_ref, file_path)])
        .current_dir(repo)
        .output()
        .ok()?;

    if output.status.success() {
        Some(String::from_utf8_lossy(&output.stdout).to_string())
    } else {
        None
    }
}

// ─── Report building ─────────────────────────────────────────────────────

fn build_report(
    repo: &Path,
    from_ref: &str,
    to_ref: &str,
    structural_changes: Vec<StructuralChange>,
    manifest_changes: Vec<ManifestChange>,
) -> AnalysisReport {
    // Group structural changes by file
    let mut file_map: std::collections::BTreeMap<std::path::PathBuf, Vec<StructuralChange>> =
        std::collections::BTreeMap::new();
    for change in structural_changes {
        // Try to determine the file from the qualified name
        let file = qualified_name_to_file(&change.qualified_name);
        file_map.entry(file).or_default().push(change);
    }

    let structural_breaking: usize = file_map
        .values()
        .flat_map(|v| v.iter())
        .filter(|c| c.is_breaking)
        .count();
    let manifest_breaking = manifest_changes.iter().filter(|c| c.is_breaking).count();
    let total_breaking = structural_breaking + manifest_breaking;
    let files_with_breaking = file_map
        .values()
        .filter(|changes| changes.iter().any(|c| c.is_breaking))
        .count();

    let changes: Vec<FileChanges> = file_map
        .into_iter()
        .map(|(file, changes)| FileChanges {
            file,
            status: FileStatus::Modified,
            structural_changes: changes,
            behavioral_changes: Vec::new(),
        })
        .collect();

    // Get SHAs
    let from_sha = resolve_sha(repo, from_ref).unwrap_or_else(|| from_ref.to_string());
    let to_sha = resolve_sha(repo, to_ref).unwrap_or_else(|| to_ref.to_string());
    let commit_count = count_commits(repo, from_ref, to_ref).unwrap_or(0);

    AnalysisReport {
        repository: repo.to_path_buf(),
        comparison: Comparison {
            from_ref: from_ref.to_string(),
            to_ref: to_ref.to_string(),
            from_sha,
            to_sha,
            commit_count,
            analysis_timestamp: chrono::Utc::now().to_rfc3339(),
        },
        summary: Summary {
            total_breaking_changes: total_breaking,
            structural_breaking_changes: structural_breaking,
            behavioral_breaking_changes: 0, // BU not yet implemented
            manifest_breaking_changes: manifest_breaking,
            files_with_breaking_changes: files_with_breaking,
        },
        changes,
        manifest_changes,
        metadata: AnalysisMetadata {
            call_graph_analysis: "none (TD-only mode)".to_string(),
            tool_version: env!("CARGO_PKG_VERSION").to_string(),
            llm_usage: None,
        },
    }
}

// ─── Git helpers ─────────────────────────────────────────────────────────

fn resolve_sha(repo: &Path, git_ref: &str) -> Option<String> {
    let output = std::process::Command::new("git")
        .args(["rev-parse", git_ref])
        .current_dir(repo)
        .output()
        .ok()?;
    if output.status.success() {
        Some(String::from_utf8_lossy(&output.stdout).trim().to_string())
    } else {
        None
    }
}

fn count_commits(repo: &Path, from_ref: &str, to_ref: &str) -> Option<usize> {
    let output = std::process::Command::new("git")
        .args(["rev-list", "--count", &format!("{}..{}", from_ref, to_ref)])
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

// ─── Output helpers ──────────────────────────────────────────────────────

fn write_json_output(value: &impl serde::Serialize, output: Option<&Path>) -> Result<()> {
    let json = serde_json::to_string_pretty(value)?;
    if let Some(path) = output {
        std::fs::write(path, &json)
            .with_context(|| format!("Failed to write output to {}", path.display()))?;
        eprintln!("Output written to {}", path.display());
    } else {
        println!("{}", json);
    }
    Ok(())
}

fn count_unique_files(surface: &ApiSurface) -> usize {
    let files: std::collections::HashSet<&std::path::Path> =
        surface.symbols.iter().map(|s| s.file.as_path()).collect();
    files.len()
}

/// Convert a qualified name like "src/api/users.createUser" to a file path.
fn qualified_name_to_file(qualified_name: &str) -> std::path::PathBuf {
    if let Some(dot_pos) = qualified_name.rfind('.') {
        let file_part = &qualified_name[..dot_pos];
        std::path::PathBuf::from(format!("{}.d.ts", file_part))
    } else {
        std::path::PathBuf::from(qualified_name)
    }
}

// ─── Tests ───────────────────────────────────────────────────────────────

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn qualified_name_to_file_simple() {
        assert_eq!(
            qualified_name_to_file("test.greet"),
            std::path::PathBuf::from("test.d.ts")
        );
    }

    #[test]
    fn qualified_name_to_file_nested() {
        assert_eq!(
            qualified_name_to_file("src/api/users.createUser"),
            std::path::PathBuf::from("src/api/users.d.ts")
        );
    }

    #[test]
    fn qualified_name_to_file_class_member() {
        // "test.Foo.bar" → last dot separates the member name
        assert_eq!(
            qualified_name_to_file("test.Foo.bar"),
            std::path::PathBuf::from("test.Foo.d.ts")
        );
    }

    #[test]
    fn build_report_empty() {
        let report = build_report(
            Path::new("/tmp/repo"),
            "v1.0.0",
            "v2.0.0",
            vec![],
            vec![],
        );
        assert_eq!(report.summary.total_breaking_changes, 0);
        assert!(report.changes.is_empty());
        assert!(report.manifest_changes.is_empty());
    }

    #[test]
    fn build_report_counts_breaking() {
        let changes = vec![
            StructuralChange {
                symbol: "foo".into(),
                qualified_name: "test.foo".into(),
                kind: "Function".into(),
                change_type: StructuralChangeType::SymbolRemoved,
                before: None,
                after: None,
                description: "removed".into(),
                is_breaking: true,
                impact: None,
            },
            StructuralChange {
                symbol: "bar".into(),
                qualified_name: "test.bar".into(),
                kind: "Function".into(),
                change_type: StructuralChangeType::SymbolAdded,
                before: None,
                after: None,
                description: "added".into(),
                is_breaking: false,
                impact: None,
            },
        ];
        let manifest = vec![ManifestChange {
            field: "type".into(),
            change_type: semver_analyzer_core::ManifestChangeType::ModuleSystemChanged,
            before: Some("commonjs".into()),
            after: Some("module".into()),
            description: "CJS to ESM".into(),
            is_breaking: true,
        }];

        let report = build_report(
            Path::new("/tmp/repo"),
            "v1",
            "v2",
            changes,
            manifest,
        );
        assert_eq!(report.summary.structural_breaking_changes, 1);
        assert_eq!(report.summary.manifest_breaking_changes, 1);
        assert_eq!(report.summary.total_breaking_changes, 2);
        assert_eq!(report.summary.files_with_breaking_changes, 1);
    }
}
