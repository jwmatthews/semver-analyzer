mod cli;
mod orchestrator;

use semver_analyzer_ts::konveyor;

use anyhow::{Context, Result};
use clap::Parser;
use std::collections::HashMap;
use std::fs::{read_to_string, write};
use std::path::Path;
use std::sync::Arc;

use cli::{AnalyzeLanguage, Cli, Command, ExtractLanguage, KonveyorLanguage};
use semver_analyzer_core::cli::DiffArgs;
use semver_analyzer_core::diff::diff_surfaces;
use semver_analyzer_core::traits::Language;
use semver_analyzer_core::{
    AnalysisReport, AnalysisSummary, ApiSurface, BehavioralChange, ChangeTypeCounts,
    ReportEnvelope, StructuralChange, StructuralChangeType,
};
use semver_analyzer_llm::LlmBehaviorAnalyzer;
use semver_analyzer_ts::cli::{TsAnalyzeArgs, TsExtractArgs, TsKonveyorArgs};
use semver_analyzer_ts::report::{count_unique_files, extract_suffix_renames};
use semver_analyzer_ts::TypeScript;

#[tokio::main]
async fn main() -> Result<()> {
    let cli = Cli::parse();

    match cli.command {
        Command::Extract { language } => match language {
            ExtractLanguage::Typescript(args) => cmd_extract_ts(args)?,
        },

        Command::Diff(args) => cmd_diff(args)?,

        Command::Analyze { language } => match language {
            AnalyzeLanguage::Typescript(args) => cmd_analyze_ts(args).await?,
        },

        Command::Konveyor { language } => match language {
            KonveyorLanguage::Typescript(args) => cmd_konveyor_ts(args).await?,
        },

        Command::Serve => {
            eprintln!("MCP server not yet implemented");
        }
    }

    Ok(())
}

// ─── Extract command (TypeScript) ───────────────────────────────────────

fn cmd_extract_ts(args: TsExtractArgs) -> Result<()> {
    let common = &args.common;
    eprintln!(
        "Extracting API surface from {} at ref {}",
        common.repo.display(),
        common.git_ref
    );

    let ts = TypeScript::new(args.build_command);
    let surface = ts
        .extract(&common.repo, &common.git_ref)
        .context("Failed to extract API surface")?;

    eprintln!(
        "Extracted {} symbols from {} files",
        surface.symbols.len(),
        count_unique_files(&surface)
    );

    write_json_output(&surface, common.output.as_deref())?;
    Ok(())
}

// ─── Diff command (language-agnostic) ───────────────────────────────────

fn cmd_diff(args: DiffArgs) -> Result<()> {
    eprintln!("Diffing {} vs {}", args.from.display(), args.to.display());

    let old_json = read_to_string(&args.from)
        .with_context(|| format!("Failed to read {}", args.from.display()))?;
    let new_json = read_to_string(&args.to)
        .with_context(|| format!("Failed to read {}", args.to.display()))?;

    let old: ApiSurface = serde_json::from_str(&old_json)
        .with_context(|| format!("Failed to parse {} as ApiSurface", args.from.display()))?;
    let new: ApiSurface = serde_json::from_str(&new_json)
        .with_context(|| format!("Failed to parse {} as ApiSurface", args.to.display()))?;

    let changes = diff_surfaces(&old, &new);

    let breaking = changes.iter().filter(|c| c.is_breaking).count();
    let non_breaking = changes.len() - breaking;
    eprintln!(
        "Found {} changes ({} breaking, {} non-breaking)",
        changes.len(),
        breaking,
        non_breaking
    );

    write_json_output(&changes, args.output.as_deref())?;
    Ok(())
}

// ─── Analyze command (TypeScript) ───────────────────────────────────────

async fn cmd_analyze_ts(args: TsAnalyzeArgs) -> Result<()> {
    let common = &args.common;
    eprintln!(
        "Analyzing {} from {} to {}",
        common.repo.display(),
        common.from,
        common.to
    );
    if common.no_llm {
        eprintln!("Mode: static analysis only (--no-llm)");
    }

    let analyzer = orchestrator::Analyzer {
        lang: Arc::new(TypeScript::new(args.build_command)),
    };
    let result = analyzer
        .run(
            &common.repo,
            &common.from,
            &common.to,
            common.no_llm,
            common.llm_command.as_deref(),
            None, // build_command already on TypeScript
            common.llm_all_files,
        )
        .await?;

    // Print summary stats
    let manifest_breaking = result
        .manifest_changes
        .iter()
        .filter(|c| c.is_breaking)
        .count();
    if !result.manifest_changes.is_empty() {
        eprintln!(
            "[TD]   {} manifest changes ({} breaking)",
            result.manifest_changes.len(),
            manifest_breaking
        );
    }

    // Build report (includes composition changes + hierarchy enrichment)
    let mut report =
        <TypeScript as Language>::build_report(&result, &common.repo, &common.from, &common.to);

    // ── Infer CSS suffix renames via LLM ─────────────────────────────
    if !common.no_llm {
        if let Some(ref llm_cmd) = common.llm_command {
            let (removed_suffixes, added_suffixes) =
                konveyor::extract_suffix_inventory(&report);
            if !removed_suffixes.is_empty() && !added_suffixes.is_empty() {
                eprintln!(
                    "[Suffix] Extracted {} removed, {} added suffixes from token diffs",
                    removed_suffixes.len(),
                    added_suffixes.len()
                );

                let suffix_result = tokio::task::spawn_blocking({
                    let cmd = llm_cmd.clone();
                    let removed: Vec<String> = removed_suffixes.into_iter().collect();
                    let added: Vec<String> = added_suffixes.into_iter().collect();
                    move || {
                        let analyzer = LlmBehaviorAnalyzer::new(&cmd);
                        let removed_refs: Vec<&str> =
                            removed.iter().map(|s| s.as_str()).collect();
                        let added_refs: Vec<&str> =
                            added.iter().map(|s| s.as_str()).collect();
                        analyzer.infer_suffix_renames(&removed_refs, &added_refs)
                    }
                })
                .await;

                match suffix_result {
                    Ok(Ok(renames)) if !renames.is_empty() => {
                        eprintln!(
                            "[Suffix] LLM identified {} CSS suffix renames:",
                            renames.len()
                        );
                        let suffix_map: HashMap<String, String> = renames
                            .iter()
                            .map(|r| {
                                eprintln!("  {} → {}", r.from, r.to);
                                (r.from.clone(), r.to.clone())
                            })
                            .collect();

                        let member_renames =
                            konveyor::apply_suffix_renames(&report, &suffix_map);

                        if !member_renames.is_empty() {
                            eprintln!(
                                "[Suffix] Applied suffix mappings: {} member renames",
                                member_renames.len()
                            );
                            report.member_renames = member_renames;
                        }
                    }
                    Ok(Ok(_)) => {
                        eprintln!("[Suffix] LLM returned no suffix renames");
                    }
                    Ok(Err(e)) => {
                        eprintln!("[Suffix] WARN: LLM suffix inference failed: {}", e);
                    }
                    Err(e) => {
                        eprintln!("[Suffix] WARN: spawn_blocking failed: {}", e);
                    }
                }
            }
        }
    }

    let total_breaking = report.summary.total_breaking_changes;
    eprintln!();
    if total_breaking == 0 {
        eprintln!("No breaking changes detected.");
    } else {
        eprintln!(
            "BREAKING: {} total breaking change(s) detected.",
            total_breaking
        );
        eprintln!(
            "  {} API changes, {} behavioral changes",
            report.summary.breaking_api_changes,
            report.summary.breaking_behavioral_changes
        );
    }

    write_json_output(&report, common.output.as_deref())?;
    Ok(())
}

// ─── Konveyor command (TypeScript) ──────────────────────────────────────

async fn cmd_konveyor_ts(args: TsKonveyorArgs) -> Result<()> {
    let common = &args.common;

    let mut rename_patterns = if let Some(ref path) = common.rename_patterns {
        konveyor::RenamePatterns::load(path)?
    } else {
        konveyor::RenamePatterns::empty()
    };

    let mut report = if let Some(ref report_path) = common.from_report {
        eprintln!("Loading report from {}", report_path.display());
        let json = read_to_string(report_path)
            .with_context(|| format!("Failed to read {}", report_path.display()))?;
        let report: AnalysisReport<TypeScript> = serde_json::from_str(&json).with_context(
            || format!("Failed to parse {} as AnalysisReport", report_path.display()),
        )?;
        report
    } else {
        let repo = common
            .repo
            .as_ref()
            .context("--repo is required when --from-report is not provided")?;
        let from = common
            .from
            .as_ref()
            .context("--from is required when --from-report is not provided")?;
        let to = common
            .to
            .as_ref()
            .context("--to is required when --from-report is not provided")?;

        eprintln!("Analyzing {} from {} to {}", repo.display(), from, to);
        if common.no_llm {
            eprintln!("Mode: static analysis only (--no-llm)");
        }

        let analyzer = orchestrator::Analyzer {
            lang: Arc::new(TypeScript::new(args.build_command.clone())),
        };
        let result = analyzer
            .run(
                repo,
                from,
                to,
                common.no_llm,
                common.llm_command.as_deref(),
                None, // build_command already on TypeScript
                common.llm_all_files,
            )
            .await?;

        <TypeScript as Language>::build_report(&result, repo, from, to)
    };

    // Build package info cache
    let pkg_info_cache = konveyor::build_package_info_cache(&report);
    let pkg_cache: HashMap<String, String> = pkg_info_cache
        .iter()
        .map(|(k, v)| (k.clone(), v.name.clone()))
        .collect();

    // Analyze token members
    let (covered_symbols, mut member_renames) =
        konveyor::analyze_token_members(&report, &rename_patterns);
    for (k, v) in &report.member_renames {
        member_renames.entry(k.clone()).or_insert_with(|| v.clone());
    }
    if !covered_symbols.is_empty() {
        eprintln!(
            "Found {} token member keys covered by parent objects, {} member renames",
            covered_symbols.len(),
            member_renames.len()
        );
    }

    // Store member renames into the report
    if !member_renames.is_empty() {
        report.member_renames = member_renames.clone();

        let suffix_renames = extract_suffix_renames(&member_renames);
        if !suffix_renames.is_empty() {
            for pkg in &mut report.packages {
                for group in &mut pkg.constants {
                    if group.strategy_hint == "CssVariablePrefix" {
                        group.suffix_renames = suffix_renames.clone();
                    }
                }
            }
        }
    }

    // Enrich package entries with npm package names and versions
    for pkg in &mut report.packages {
        if let Some(info) = pkg_info_cache.get(&pkg.name) {
            pkg.name = info.name.clone();
            pkg.old_version = info.version.clone();
        }
    }

    // Merge LLM-inferred constant rename patterns
    if let Some(ref inferred) = report.inferred_rename_patterns {
        for pat in &inferred.constant_patterns {
            rename_patterns.add_pattern(&pat.match_regex, &pat.replace);
        }
        if !inferred.constant_patterns.is_empty() {
            eprintln!(
                "Merged {} LLM-inferred constant rename patterns into rename_patterns",
                inferred.constant_patterns.len()
            );
        }
    }

    // Generate rules
    let raw_rules = konveyor::generate_rules(
        &report,
        &args.file_pattern,
        &pkg_cache,
        &rename_patterns,
        &member_renames,
    );
    let raw_count = raw_rules.len();

    // Suppress redundant rules
    let filtered_rules = konveyor::suppress_redundant_token_rules(raw_rules, &covered_symbols);

    let rules = if common.no_consolidate {
        filtered_rules
    } else {
        let (consolidated, _id_mapping) = konveyor::consolidate_rules(filtered_rules);
        eprintln!(
            "Consolidated {} rules → {} rules",
            raw_count,
            consolidated.len()
        );
        consolidated
    };

    let rules = konveyor::suppress_redundant_prop_rules(rules);
    let rules = konveyor::suppress_redundant_prop_value_rules(rules);

    let mut strategies = konveyor::extract_fix_strategies(&rules);

    // Generate dependency update rules
    let (dep_update_rules, dep_update_strategies) =
        konveyor::generate_dependency_update_rules(&report, &pkg_info_cache);
    strategies.extend(dep_update_strategies);

    let mut all_rules = rules;
    all_rules.extend(dep_update_rules);

    let fix_guidance =
        konveyor::generate_fix_guidance(&report, &all_rules, &args.file_pattern);
    let rule_count = all_rules.len();

    // Write output
    konveyor::write_ruleset_dir(
        &common.output_dir,
        &args.ruleset_name,
        &report,
        &all_rules,
    )?;

    let fix_dir = konveyor::write_fix_guidance_dir(&common.output_dir, &fix_guidance)?;
    konveyor::write_fix_strategies(&fix_dir, &strategies)?;

    // Generate conformance rules
    let conformance_rules = konveyor::generate_conformance_rules(&report);
    if !conformance_rules.is_empty() {
        let conformance_strategies = konveyor::extract_fix_strategies(&conformance_rules);
        konveyor::write_conformance_rules(&common.output_dir, &conformance_rules)?;
        strategies.extend(conformance_strategies);
        konveyor::write_fix_strategies(&fix_dir, &strategies)?;
    }

    eprintln!(
        "Generated {} Konveyor rules in {}",
        rule_count,
        common.output_dir.display()
    );
    eprintln!(
        "  Ruleset:  {}/ruleset.yaml",
        common.output_dir.display()
    );
    eprintln!(
        "  Rules:    {}/breaking-changes.yaml",
        common.output_dir.display()
    );
    if !conformance_rules.is_empty() {
        eprintln!(
            "  Conformance: {}/conformance-rules.yaml ({} rules)",
            common.output_dir.display(),
            conformance_rules.len(),
        );
    }
    eprintln!("  Fixes:    {}/fix-guidance.yaml", fix_dir.display());
    eprintln!(
        "  Strategies: {}/fix-strategies.json ({} entries)",
        fix_dir.display(),
        strategies.len()
    );
    eprintln!(
        "  Summary:  {} auto-fixable, {} need review, {} manual only",
        fix_guidance.summary.auto_fixable,
        fix_guidance.summary.needs_review,
        fix_guidance.summary.manual_only,
    );
    eprintln!();
    eprintln!(
        "Use with: konveyor-analyzer --rules {}",
        common.output_dir.display()
    );

    Ok(())
}

// ─── ReportEnvelope production ──────────────────────────────────────────

/// Build a language-agnostic `ReportEnvelope` from a typed `AnalysisReport<L>`.
#[allow(dead_code)]
fn build_envelope<L: Language>(
    report: &AnalysisReport<L>,
    structural_changes: &[StructuralChange],
) -> anyhow::Result<ReportEnvelope> {
    let summary = AnalysisSummary {
        total_structural_breaking: structural_changes
            .iter()
            .filter(|c| c.is_breaking)
            .count(),
        total_structural_non_breaking: structural_changes
            .iter()
            .filter(|c| !c.is_breaking)
            .count(),
        total_behavioral_changes: report
            .changes
            .iter()
            .map(|fc| fc.breaking_behavioral_changes.len())
            .sum(),
        total_manifest_changes: report.manifest_changes.len(),
        packages_analyzed: report.packages.len(),
        files_changed: report.changes.len(),
        by_change_type: count_change_types(structural_changes),
    };

    let behavioral_changes: Vec<&BehavioralChange<L>> = report
        .changes
        .iter()
        .flat_map(|fc| fc.breaking_behavioral_changes.iter())
        .collect();

    let language_report_value = serde_json::json!({
        "behavioral_changes": serde_json::to_value(&behavioral_changes)
            .unwrap_or(serde_json::Value::Array(vec![])),
        "manifest_changes": serde_json::to_value(&report.manifest_changes)
            .unwrap_or(serde_json::Value::Array(vec![])),
    });

    Ok(ReportEnvelope {
        language: L::NAME.to_string(),
        version: env!("CARGO_PKG_VERSION").to_string(),
        summary,
        structural_changes: structural_changes.to_vec(),
        language_report: language_report_value,
    })
}

#[allow(dead_code)]
fn count_change_types(structural_changes: &[StructuralChange]) -> ChangeTypeCounts {
    let mut counts = ChangeTypeCounts::default();
    for change in structural_changes {
        match &change.change_type {
            StructuralChangeType::Added(_) => counts.added += 1,
            StructuralChangeType::Removed(_) => counts.removed += 1,
            StructuralChangeType::Changed(_) => counts.changed += 1,
            StructuralChangeType::Renamed { .. } => counts.renamed += 1,
            StructuralChangeType::Relocated { .. } => counts.relocated += 1,
        }
    }
    counts
}

// ─── Output helpers ─────────────────────────────────────────────────────

fn write_json_output(value: &impl serde::Serialize, output: Option<&Path>) -> Result<()> {
    let json = serde_json::to_string_pretty(value)?;
    if let Some(path) = output {
        write(path, &json)
            .with_context(|| format!("Failed to write output to {}", path.display()))?;
        eprintln!("Output written to {}", path.display());
    } else {
        println!("{}", json);
    }
    Ok(())
}
