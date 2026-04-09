//! CLI error rendering and degradation summary display.
//!
//! This module provides the user-facing error presentation layer:
//!
//! - [`render_error`] — Renders fatal errors with colors, causal chain, and tips
//! - [`print_degradation_summary`] — Shows end-of-run warnings for non-fatal issues
//!
//! All error formatting goes through this module. Never use `eprintln!`
//! directly for error output in production code.

use owo_colors::OwoColorize;
use semver_analyzer_core::diagnostics::DegradationTracker;
use semver_analyzer_core::error::DiagnosedError;

use crate::progress::ProgressReporter;

/// Render a fatal error with colors, causal chain, and remediation tips.
///
/// This is the primary error display function. It:
/// 1. Walks the `anyhow` chain for `Diagnosed` markers (single downcast)
/// 2. Renders colored output: red `error:`, dimmed `caused by:`, cyan `tip:`
/// 3. Falls back to pattern-matching on error text for undiagnosed errors
pub fn render_error(err: &anyhow::Error) {
    let tip = extract_tip(err);

    // Primary error (red, bold)
    eprintln!("\n{} {}", "error:".red().bold(), err);

    // Causal chain (dimmed, indented) — skip the first (already printed)
    // and skip empty Display strings (Diagnosed markers)
    for cause in err.chain().skip(1) {
        let msg = cause.to_string();
        if msg.is_empty() {
            continue; // Skip Diagnosed markers
        }
        eprintln!("  {} {}", "caused by:".dimmed(), msg);
    }

    // Remediation tip (cyan)
    if let Some(tip) = tip {
        eprintln!();
        for (i, line) in tip.lines().enumerate() {
            if i == 0 {
                eprintln!("  {} {}", "tip:".cyan().bold(), line);
            } else {
                eprintln!("       {}", line);
            }
        }
    }

    eprintln!();
}

/// Walk the anyhow error chain looking for `Diagnosed` markers.
///
/// The `Diagnosed` wrapper is added by `.diagnose()` or `.with_diagnosis()`
/// at error boundaries. This function performs a single `downcast_ref` —
/// no per-language-type dispatch needed.
fn extract_tip(err: &anyhow::Error) -> Option<String> {
    // Check the outermost error first (DiagnosedError from .diagnose())
    if let Some(d) = err.downcast_ref::<DiagnosedError>() {
        let tip = d.tip();
        if !tip.is_empty() {
            return Some(tip.to_string());
        }
    }
    // Walk the chain for nested DiagnosedError markers
    for cause in err.chain().skip(1) {
        if let Some(d) = cause.downcast_ref::<DiagnosedError>() {
            let tip = d.tip();
            if !tip.is_empty() {
                return Some(tip.to_string());
            }
        }
    }
    // Fallback: pattern-match on common error messages
    pattern_match_tip(err)
}

/// Last-resort pattern matching for errors without `Diagnosed` markers.
///
/// Catches common OS-level errors that may not have been wrapped with
/// a diagnosis at the call site.
fn pattern_match_tip(err: &anyhow::Error) -> Option<String> {
    let msg = format!("{:#}", err);
    if msg.contains("not a git repository") || msg.contains("Not a git repository") {
        return Some("Check that --repo points to a valid git repository root.".into());
    }
    if msg.contains("Permission denied") {
        return Some("Check file permissions for the target path.".into());
    }
    if msg.contains("No space left on device") {
        return Some("Free up disk space and retry.".into());
    }
    if msg.contains("command not found") || msg.contains("No such file or directory") {
        return Some(
            "A required command was not found. Check that git and Node.js are installed.".into(),
        );
    }
    None
}

/// Print an end-of-run summary of non-fatal degradation issues.
///
/// Called at the end of `cmd_analyze_ts` and `cmd_konveyor_ts` to inform
/// the user about parts of the analysis that may be incomplete.
pub fn print_degradation_summary(tracker: &DegradationTracker, reporter: &ProgressReporter) {
    let issues = tracker.issues();
    if issues.is_empty() {
        return;
    }

    reporter.println("");
    reporter.println(&format!(
        "{} Analysis completed with {} warning(s):",
        "warning:".yellow().bold(),
        issues.len()
    ));
    for issue in &issues {
        reporter.println(&format!(
            "  {} [{}] {} — {}",
            "•".dimmed(),
            issue.phase,
            issue.message,
            issue.impact.dimmed()
        ));
    }
}
