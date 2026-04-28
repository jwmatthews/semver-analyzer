//! Git worktree management for Java extraction.
//!
//! Provides a `WorktreeGuard` that wraps the core's git-only guard with
//! optional Maven/Gradle build support. When a build command is configured
//! (via `JavaRefBuildConfig`), the guard runs it after checkout to generate
//! compiled artifacts, resolve dependencies, and produce generated sources.
//!
//! When no build command is configured, Java source files are parsed
//! directly with tree-sitter — no build step needed.

mod error;
mod guard;

pub use error::{ExtractionWarning, JavaWorktreeError};
pub use guard::JavaWorktreeGuard;

/// Per-ref build configuration for Java extraction.
///
/// Optionally configures Maven/Gradle build commands and JDK paths
/// for each git ref being analyzed. When provided, the worktree guard
/// runs the build after checkout.
#[derive(Debug, Clone, Default)]
pub struct JavaRefBuildConfig {
    /// Path to the JDK to use (e.g., `/usr/lib/jvm/java-17`).
    /// When set, `JAVA_HOME` is set in the build environment.
    pub java_home: Option<std::path::PathBuf>,

    /// Custom build command to run after checkout.
    /// Examples: `"mvn compile -DskipTests"`, `"gradle compileJava"`.
    /// When not set, the guard auto-detects Maven or Gradle and runs
    /// a default compile command.
    pub build_command: Option<String>,

    /// Whether to skip the build step entirely.
    /// When true, source files are parsed directly (fastest, default).
    pub skip_build: bool,
}
