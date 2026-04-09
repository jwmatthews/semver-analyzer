//! Git worktree management with RAII cleanup.
//!
//! Manages temporary worktrees for checking out git refs, installing
//! dependencies, and running tsc. Ensures cleanup on drop, panic, or SIGINT.

mod error;
mod guard;
mod package_manager;
mod tsc;

pub use error::WorktreeError;
pub use guard::WorktreeGuard;
pub use package_manager::PackageManager;

/// Non-fatal issues encountered during worktree setup.
///
/// These are captured on [`WorktreeGuard`] via `guard.warnings()` and
/// propagated to the `DegradationTracker` by the caller of `extract()`.
/// The per-package tsc failures stay as `tracing::warn!` for `--log-file`
/// visibility; only the aggregate outcome is captured here.
#[derive(Debug, Clone)]
pub enum ExtractionWarning {
    /// tsc partially succeeded — some packages compiled, others failed.
    /// The project build fallback also failed.
    PartialTscBuildFailed {
        succeeded: usize,
        failed: usize,
        build_error: String,
    },

    /// tsc completely failed but the project build succeeded as fallback.
    TscFailedBuildSucceeded { tsc_error: String },
}
