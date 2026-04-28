//! Java worktree guard with optional build support.
//!
//! Composes the core `WorktreeGuard` (git-only) with Java-specific
//! build steps (Maven/Gradle detection and execution).

use super::error::{ExtractionWarning, JavaWorktreeError};
use super::JavaRefBuildConfig;
use anyhow::{Context, Result};
use semver_analyzer_core::traits::WorktreeAccess;
use std::path::Path;
use std::process::Command;

/// RAII guard for a Java git worktree with optional build support.
///
/// On construction:
/// 1. Creates a git worktree at the specified ref (via core's `WorktreeGuard`).
/// 2. If a build command is configured, detects Maven/Gradle and runs it.
/// 3. If build fails, records a warning and continues (source-only extraction).
///
/// On drop, the worktree is automatically cleaned up.
pub struct JavaWorktreeGuard {
    inner: semver_analyzer_core::git::WorktreeGuard,
    warnings: Vec<ExtractionWarning>,
}

impl JavaWorktreeGuard {
    /// Create a new worktree with optional build support.
    pub fn new(repo: &Path, git_ref: &str, config: &JavaRefBuildConfig) -> Result<Self> {
        let inner = semver_analyzer_core::git::WorktreeGuard::new(repo, git_ref)
            .map_err(|e| {
                let msg = e.to_string();
                if msg.contains("Not a git repository") {
                    JavaWorktreeError::NotAGitRepo {
                        path: repo.display().to_string(),
                    }
                } else if msg.contains("not a valid") || msg.contains("unknown revision") {
                    JavaWorktreeError::RefNotFound {
                        git_ref: git_ref.to_string(),
                    }
                } else {
                    JavaWorktreeError::WorktreeCreationFailed {
                        reason: msg,
                    }
                }
            })
            .context("Failed to create Java worktree")?;

        let mut guard = Self {
            inner,
            warnings: Vec::new(),
        };

        // Run build if configured and not skipped
        if !config.skip_build && config.build_command.is_some() {
            guard.run_build(config);
        }

        Ok(guard)
    }

    /// Create a worktree without any build step (source-only).
    ///
    /// Equivalent to `new()` with `skip_build: true`.
    pub fn create_only(repo: &Path, git_ref: &str) -> Result<Self> {
        Self::new(
            repo,
            git_ref,
            &JavaRefBuildConfig {
                skip_build: true,
                ..Default::default()
            },
        )
    }

    /// Path to the worktree directory.
    pub fn path(&self) -> &Path {
        self.inner.path()
    }

    /// Non-fatal warnings accumulated during setup.
    pub fn warnings(&self) -> &[ExtractionWarning] {
        &self.warnings
    }

    /// Run the configured or auto-detected build command.
    fn run_build(&mut self, config: &JavaRefBuildConfig) {
        let worktree = self.inner.path();

        let build_cmd = match &config.build_command {
            Some(cmd) => cmd.clone(),
            None => {
                // Auto-detect build system
                if worktree.join("pom.xml").exists() {
                    "mvn compile -DskipTests -q".to_string()
                } else if worktree.join("build.gradle").exists()
                    || worktree.join("build.gradle.kts").exists()
                {
                    "gradle compileJava -q".to_string()
                } else {
                    tracing::debug!("No build file found, skipping build");
                    return;
                }
            }
        };

        tracing::info!(command = %build_cmd, "Running Java build");

        let mut cmd = Command::new("sh");
        cmd.args(["-c", &build_cmd]).current_dir(worktree);

        // Set JAVA_HOME if configured
        if let Some(java_home) = &config.java_home {
            if java_home.exists() {
                cmd.env("JAVA_HOME", java_home);
            } else {
                tracing::warn!(
                    path = %java_home.display(),
                    "JAVA_HOME path does not exist, using system default"
                );
            }
        }

        match cmd.output() {
            Ok(output) if output.status.success() => {
                tracing::info!("Java build succeeded");
            }
            Ok(output) => {
                let stderr = String::from_utf8_lossy(&output.stderr).to_string();
                tracing::warn!(
                    command = %build_cmd,
                    "Build failed, continuing with source-only extraction"
                );
                self.warnings.push(ExtractionWarning::BuildFailedSourceOnly {
                    build_error: stderr,
                });
            }
            Err(e) => {
                tracing::warn!(
                    command = %build_cmd,
                    error = %e,
                    "Failed to execute build command"
                );
                self.warnings.push(ExtractionWarning::BuildFailedSourceOnly {
                    build_error: e.to_string(),
                });
            }
        }
    }
}

impl WorktreeAccess for JavaWorktreeGuard {
    fn path(&self) -> &Path {
        self.inner.path()
    }
}
