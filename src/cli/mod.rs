//! CLI argument parsing and command dispatch.

use clap::{Parser, Subcommand};
use semver_analyzer_core::cli::DiffArgs;
use semver_analyzer_ts::cli::{TsAnalyzeArgs, TsExtractArgs, TsKonveyorArgs};

/// Semantic Breaking Change Analyzer
///
/// Deterministic, structured analysis of breaking changes between two git refs.
/// Combines static API surface extraction with optional LLM-based behavioral analysis.
#[derive(Parser, Debug)]
#[command(name = "semver-analyzer", version, about)]
pub struct Cli {
    #[command(subcommand)]
    pub command: Command,
}

#[derive(Subcommand, Debug)]
pub enum Command {
    /// Full pipeline: extract -> diff -> impact -> behavioral analysis.
    Analyze {
        #[command(subcommand)]
        language: AnalyzeLanguage,
    },

    /// Extract API surface from source code at a specific ref.
    Extract {
        #[command(subcommand)]
        language: ExtractLanguage,
    },

    /// Compare two API surfaces and identify structural changes.
    ///
    /// This command is language-agnostic — it compares two JSON surface
    /// files using minimal semantics (no language-specific rules).
    Diff(DiffArgs),

    /// Generate Konveyor analyzer rules from breaking change analysis.
    Konveyor {
        #[command(subcommand)]
        language: KonveyorLanguage,
    },

    /// Start as an MCP server (stdio transport).
    Serve,
}

/// Language-specific subcommands for the `analyze` action.
#[derive(Subcommand, Debug)]
pub enum AnalyzeLanguage {
    /// Analyze a TypeScript/JavaScript project.
    Typescript(TsAnalyzeArgs),
}

/// Language-specific subcommands for the `extract` action.
#[derive(Subcommand, Debug)]
pub enum ExtractLanguage {
    /// Extract API surface from a TypeScript/JavaScript project.
    Typescript(TsExtractArgs),
}

/// Language-specific subcommands for the `konveyor` action.
#[derive(Subcommand, Debug)]
pub enum KonveyorLanguage {
    /// Generate Konveyor rules for a TypeScript/JavaScript project.
    Typescript(TsKonveyorArgs),
}
