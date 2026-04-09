//! Error types for worktree operations.

use semver_analyzer_core::error::ErrorTip;
use std::path::PathBuf;
use thiserror::Error;

/// Errors that can occur during worktree management.
#[derive(Debug, Error)]
pub enum WorktreeError {
    #[error("Not a git repository: {path}")]
    NotAGitRepo { path: PathBuf },

    #[error("Git ref does not exist: {git_ref}")]
    RefNotFound { git_ref: String },

    #[error("Failed to create git worktree at {path}: {reason}")]
    WorktreeCreationFailed { path: PathBuf, reason: String },

    #[error("Failed to remove git worktree at {path}: {reason}")]
    WorktreeRemovalFailed { path: PathBuf, reason: String },

    #[error("No lockfile found at ref {git_ref}. Expected one of: package-lock.json, yarn.lock, pnpm-lock.yaml")]
    NoLockfileFound { git_ref: String },

    #[error("Package install failed ({command}): {reason}")]
    PackageInstallFailed { command: String, reason: String },

    #[error("No tsconfig.json found at ref {git_ref}")]
    NoTsconfigFound { git_ref: String },

    #[error("tsconfig.json has noEmit: true, which conflicts with --declaration. Consider adding a separate tsconfig.build.json")]
    NoEmitConflict,

    #[error("tsc --declaration failed with {error_count} errors at ref {git_ref}: {reason}")]
    TscFailed {
        git_ref: String,
        error_count: usize,
        reason: String,
    },

    #[error("Dependencies not installed at ref {git_ref}. Import resolution errors in tsc output")]
    MissingDependencies { git_ref: String },

    #[error("Project references not built. Run tsc --build in the monorepo root first")]
    ProjectReferencesNotBuilt,

    #[error("Unsupported TypeScript syntax at ref {git_ref}: {reason}")]
    UnsupportedSyntax { git_ref: String, reason: String },

    #[error("Project build failed ({command}): {reason}")]
    ProjectBuildFailed { command: String, reason: String },

    #[error("Insufficient disk space: need approximately {needed_mb}MB, have {available_mb}MB")]
    InsufficientDiskSpace { needed_mb: u64, available_mb: u64 },

    #[error("Command execution failed: {0}")]
    CommandFailed(String),

    #[error(transparent)]
    Io(#[from] std::io::Error),
}

impl ErrorTip for WorktreeError {
    fn tip(&self) -> Option<String> {
        Some(match self {
            Self::NotAGitRepo { path } => format!(
                "Verify that '{}' is a git repository (contains a .git directory).\n\
                 If this is a subdirectory, point --repo to the repository root.",
                path.display()
            ),
            Self::RefNotFound { git_ref } => format!(
                "The ref '{}' was not found. Run 'git tag -l' or 'git branch -a' \
                 in the repo to see available refs.\n\
                 If using a short ref like 'v6', try the full tag name (e.g. 'v6.0.0').",
                git_ref
            ),
            Self::WorktreeCreationFailed { path, .. } => format!(
                "Failed to create a git worktree at '{}'.\n\
                 Check that the path is writable and no stale worktree exists there.\n\
                 Try running 'git worktree prune' in the repo to clean up stale entries.",
                path.display()
            ),
            Self::NoLockfileFound { .. } => {
                "The repo needs a package lockfile (package-lock.json, yarn.lock, \
                 or pnpm-lock.yaml) at this ref.\n\
                 If this ref predates lockfiles, try a later tag.\n\
                 If the project uses a different package manager, specify --build-command."
                    .to_string()
            }
            Self::PackageInstallFailed { command, .. } => format!(
                "The package install command '{}' failed.\n\
                 Try running it manually in the repo to see the full error.\n\
                 Check that your Node.js version is compatible with this project.\n\
                 Use --log-file debug.log for full output.",
                command
            ),
            Self::NoTsconfigFound { .. } => {
                "The repo needs a tsconfig.json for TypeScript declaration extraction.\n\
                 If this is a monorepo, use --build-command to specify the project's \
                 own build system that generates .d.ts files."
                    .to_string()
            }
            Self::NoEmitConflict => {
                "The tsconfig.json has 'noEmit: true' which conflicts with declaration \
                 generation.\n\
                 Options:\n\
                 - Add a tsconfig.build.json without noEmit\n\
                 - Use --build-command to specify a custom build that generates .d.ts files"
                    .to_string()
            }
            Self::TscFailed { error_count, .. } => format!(
                "TypeScript compilation failed with {} error(s).\n\
                 Common causes:\n\
                 - Missing dependencies: run 'npm ci' or 'yarn install' first\n\
                 - Incompatible TypeScript version\n\
                 - Project references not built: try 'tsc --build' in the monorepo root\n\
                 Use --log-file debug.log to see full tsc output.",
                error_count
            ),
            Self::MissingDependencies { .. } => {
                "TypeScript cannot resolve imports — dependencies are not installed.\n\
                 Ensure the package install step completed successfully.\n\
                 If using a monorepo, dependencies may need to be hoisted or linked."
                    .to_string()
            }
            Self::ProjectReferencesNotBuilt => {
                "This monorepo uses TypeScript project references.\n\
                 Run 'tsc --build' in the monorepo root to build all referenced \
                 projects, then retry."
                    .to_string()
            }
            Self::ProjectBuildFailed { command, .. } => format!(
                "The build command '{}' failed.\n\
                 Try running this command manually in the repo directory to debug.\n\
                 Check that all prerequisites (Node.js version, native build tools) are met.\n\
                 Use --log-file debug.log for full build output.",
                command
            ),
            Self::InsufficientDiskSpace {
                needed_mb,
                available_mb,
            } => format!(
                "Need approximately {}MB of disk space but only {}MB available.\n\
                 Free up disk space — each worktree needs room for node_modules \
                 and build artifacts.",
                needed_mb, available_mb
            ),
            Self::UnsupportedSyntax { .. } => {
                "The TypeScript source at this ref uses syntax that cannot be parsed.\n\
                 This may indicate a very old or experimental TypeScript version.\n\
                 Check that the ref you specified is correct."
                    .to_string()
            }
            // No actionable tip for generic command failures or IO errors
            Self::CommandFailed(_) | Self::Io(_) | Self::WorktreeRemovalFailed { .. } => {
                return None
            }
        })
    }
}
