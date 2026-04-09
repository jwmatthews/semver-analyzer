//! Shared git utility functions for the TypeScript crate.
//!
//! These functions provide consistent, trace-logged access to git
//! operations used across multiple modules. **Do not duplicate these
//! functions** — there were previously 4+ copies across the codebase.

use std::path::Path;
use std::process::Command;

/// Read a file from a git ref via `git show <ref>:<path>`.
///
/// Returns `None` if the file doesn't exist at the given ref,
/// the git command fails, or the output is not valid UTF-8.
/// All failures are logged at `trace` level for debugging with
/// `--log-level trace --log-file debug.log`.
pub fn read_git_file(repo: &Path, git_ref: &str, file_path: &str) -> Option<String> {
    let output = Command::new("git")
        .args(["show", &format!("{}:{}", git_ref, file_path)])
        .current_dir(repo)
        .output()
        .map_err(|e| {
            tracing::trace!(
                %e,
                repo = %repo.display(),
                %git_ref,
                %file_path,
                "git show failed to execute"
            );
            e
        })
        .ok()?;

    if !output.status.success() {
        tracing::trace!(
            repo = %repo.display(),
            %git_ref,
            %file_path,
            stderr = %String::from_utf8_lossy(&output.stderr).trim(),
            "git show returned non-zero"
        );
        return None;
    }

    String::from_utf8(output.stdout)
        .map_err(|e| {
            tracing::trace!(
                %e,
                %file_path,
                "git show output was not valid UTF-8"
            );
            e
        })
        .ok()
}

/// Get the diff of a single file between two refs via `git diff <from>..<to> -- <path>`.
///
/// Returns `None` if the file has no changes between the refs,
/// the git command fails, or the output is empty.
/// All failures are logged at `trace` level.
pub fn git_diff_file(repo: &Path, from_ref: &str, to_ref: &str, file_path: &str) -> Option<String> {
    let output = Command::new("git")
        .args([
            "-C",
            &repo.to_string_lossy(),
            "diff",
            &format!("{}..{}", from_ref, to_ref),
            "--",
            file_path,
        ])
        .output()
        .map_err(|e| {
            tracing::trace!(
                %e,
                repo = %repo.display(),
                %from_ref,
                %to_ref,
                %file_path,
                "git diff failed to execute"
            );
            e
        })
        .ok()?;

    if !output.status.success() {
        tracing::trace!(
            repo = %repo.display(),
            %from_ref,
            %to_ref,
            %file_path,
            stderr = %String::from_utf8_lossy(&output.stderr).trim(),
            "git diff returned non-zero"
        );
        return None;
    }

    let content = String::from_utf8_lossy(&output.stdout).to_string();
    if content.is_empty() {
        None
    } else {
        Some(content)
    }
}
