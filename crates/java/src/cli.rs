//! Java-specific CLI argument structs.

use clap::Args;
use semver_analyzer_core::cli::{CommonAnalyzeArgs, CommonExtractArgs, CommonKonveyorArgs};
use std::path::PathBuf;

/// Java-specific arguments for the `analyze` command.
#[derive(Args, Debug)]
pub struct JavaAnalyzeArgs {
    #[command(flatten)]
    pub common: CommonAnalyzeArgs,

    /// Build command to run after checkout (e.g., "mvn compile -DskipTests").
    /// Applied to both refs unless overridden by --from-build-command / --to-build-command.
    #[arg(long)]
    pub build_command: Option<String>,

    /// Build command for the from-ref only (overrides --build-command).
    #[arg(long)]
    pub from_build_command: Option<String>,

    /// Build command for the to-ref only (overrides --build-command).
    #[arg(long)]
    pub to_build_command: Option<String>,

    /// JAVA_HOME path for the from-ref.
    #[arg(long)]
    pub from_java_home: Option<PathBuf>,

    /// JAVA_HOME path for the to-ref.
    #[arg(long)]
    pub to_java_home: Option<PathBuf>,

    /// Skip the build step entirely (source-only extraction).
    #[arg(long, default_value_t = true)]
    pub skip_build: bool,
}

/// Java-specific arguments for the `extract` command.
#[derive(Args, Debug)]
pub struct JavaExtractArgs {
    #[command(flatten)]
    pub common: CommonExtractArgs,

    /// Build command to run after checkout.
    #[arg(long)]
    pub build_command: Option<String>,

    /// JAVA_HOME path.
    #[arg(long)]
    pub java_home: Option<PathBuf>,

    /// Skip the build step entirely (source-only extraction).
    #[arg(long, default_value_t = true)]
    pub skip_build: bool,
}

/// Java-specific arguments for the `konveyor` command.
#[derive(Args, Debug)]
pub struct JavaKonveyorArgs {
    #[command(flatten)]
    pub common: CommonKonveyorArgs,

    /// Project name for rule generation (e.g., "spring-boot").
    /// Used in rule IDs and ruleset metadata.
    #[arg(long)]
    pub project_name: Option<String>,

    /// Rule ID prefix (e.g., "sb4"). Derived from project-name if not set.
    #[arg(long)]
    pub rule_prefix: Option<String>,

    /// Migration guide URL to include in rule links.
    #[arg(long)]
    pub migration_guide_url: Option<String>,

    /// Namespace migration pairs (e.g., "javax.persistence=jakarta.persistence").
    /// Generates import relocation rules for entire package namespaces.
    /// Can be specified multiple times.
    #[arg(long = "namespace-migration", value_name = "OLD=NEW")]
    pub namespace_migrations: Vec<String>,
}
