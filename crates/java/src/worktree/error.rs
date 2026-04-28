//! Java worktree error types with user-facing diagnostics.

use semver_analyzer_core::error::ErrorTip;

/// Errors that can occur during Java worktree setup.
#[derive(Debug, thiserror::Error)]
pub enum JavaWorktreeError {
    #[error("not a git repository: {path}")]
    NotAGitRepo { path: String },

    #[error("git ref not found: {git_ref}")]
    RefNotFound { git_ref: String },

    #[error("no build file found (pom.xml, build.gradle, or build.gradle.kts)")]
    NoBuildFile,

    #[error("build command failed: {command}")]
    BuildFailed {
        command: String,
        stderr: String,
    },

    #[error("JAVA_HOME not found: {path}")]
    JdkNotFound { path: String },

    #[error("worktree creation failed: {reason}")]
    WorktreeCreationFailed { reason: String },

    #[error(transparent)]
    Other(#[from] anyhow::Error),
}

impl ErrorTip for JavaWorktreeError {
    fn tip(&self) -> Option<String> {
        match self {
            Self::NotAGitRepo { path } => Some(format!(
                "Ensure '{}' is a git repository. Run 'git status' to verify.",
                path
            )),
            Self::RefNotFound { git_ref } => Some(format!(
                "Verify the ref '{}' exists. Run 'git tag -l' or 'git branch -a' to list refs.",
                git_ref
            )),
            Self::NoBuildFile => Some(
                "The worktree has no pom.xml or build.gradle. \
                 If this is a source-only project, use --skip-build. \
                 If the build file is in a subdirectory, specify --build-command."
                    .into(),
            ),
            Self::BuildFailed { command, stderr } => Some(format!(
                "Build command '{}' failed. Try running it manually in the repo.\n  stderr: {}",
                command,
                stderr.lines().take(5).collect::<Vec<_>>().join("\n  ")
            )),
            Self::JdkNotFound { path } => Some(format!(
                "JAVA_HOME '{}' does not exist. Check the --java-home path.",
                path
            )),
            Self::WorktreeCreationFailed { .. } | Self::Other(_) => None,
        }
    }
}

/// Non-fatal warnings from worktree setup.
#[derive(Debug, Clone)]
pub enum ExtractionWarning {
    /// Build failed but source extraction can proceed without it.
    BuildFailedSourceOnly {
        build_error: String,
    },
    /// Multi-module build partially succeeded.
    PartialBuild {
        succeeded: usize,
        failed: usize,
    },
}
