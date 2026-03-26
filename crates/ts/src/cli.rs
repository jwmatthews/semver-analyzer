//! TypeScript-specific CLI argument structs.
//!
//! Each struct flattens the shared `Common*Args` from core and adds
//! TypeScript-specific flags like `--build-command`.

use clap::Args;
use semver_analyzer_core::cli::{CommonAnalyzeArgs, CommonExtractArgs, CommonKonveyorArgs};

/// TypeScript-specific arguments for the `analyze` command.
#[derive(Args, Debug)]
pub struct TsAnalyzeArgs {
    #[command(flatten)]
    pub common: CommonAnalyzeArgs,

    /// Custom build command to run before API extraction.
    /// Default: "yarn build"
    #[arg(long)]
    pub build_command: Option<String>,
}

/// TypeScript-specific arguments for the `extract` command.
#[derive(Args, Debug)]
pub struct TsExtractArgs {
    #[command(flatten)]
    pub common: CommonExtractArgs,

    /// Custom build command to run before API extraction.
    /// Default: "yarn build"
    #[arg(long)]
    pub build_command: Option<String>,
}

/// TypeScript-specific arguments for the `konveyor` command.
#[derive(Args, Debug)]
pub struct TsKonveyorArgs {
    #[command(flatten)]
    pub common: CommonKonveyorArgs,

    /// Custom build command to run before API extraction.
    /// Default: "yarn build"
    #[arg(long)]
    pub build_command: Option<String>,

    /// File glob pattern for filecontent rules.
    /// Determines which files Konveyor will scan for violations.
    #[arg(long, default_value = "*.{ts,tsx,js,jsx,mjs,cjs}")]
    pub file_pattern: String,

    /// Name for the generated ruleset.
    #[arg(long, default_value = "semver-breaking-changes")]
    pub ruleset_name: String,
}
