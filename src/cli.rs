//! CLI definition using clap

use clap::{Parser, Subcommand, ValueEnum};
use serde::{Deserialize, Serialize};
use std::path::PathBuf;

/// Output format for results
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq, ValueEnum, Serialize, Deserialize)]
#[serde(rename_all = "lowercase")]
pub enum OutputFormat {
    #[default]
    Table,
    Json,
}

impl std::fmt::Display for OutputFormat {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            OutputFormat::Table => write!(f, "table"),
            OutputFormat::Json => write!(f, "json"),
        }
    }
}

#[derive(Parser)]
#[command(name = "tonsuu-checker")]
#[command(author = "yuuji")]
#[command(version)]
#[command(about = "Dump truck cargo weight estimation using AI image analysis")]
#[command(long_about = None)]
pub struct Cli {
    #[command(subcommand)]
    pub command: Commands,

    /// AI backend to use (gemini, claude, codex)
    #[arg(long, global = true)]
    pub backend: Option<String>,

    /// Model name override
    #[arg(long, global = true)]
    pub model: Option<String>,

    /// Output format (json, table). Uses config value if not specified.
    #[arg(long, short = 'o', global = true)]
    pub output: Option<OutputFormat>,

    /// Verbose output
    #[arg(long, short = 'v', global = true)]
    pub verbose: bool,
}

#[derive(Subcommand)]
pub enum Commands {
    /// Analyze a single image
    Analyze {
        /// Path to image file
        image: PathBuf,

        /// Skip cache lookup (overrides config)
        #[arg(long)]
        no_cache: bool,

        /// Number of ensemble samples. Uses config value if not specified.
        #[arg(long, short = 'n')]
        ensemble: Option<u32>,
    },

    /// Batch analyze images in a folder
    Batch {
        /// Path to folder containing images
        folder: PathBuf,

        /// Output file for results
        #[arg(long, short = 'o')]
        output: Option<PathBuf>,

        /// Skip cache lookup (overrides config)
        #[arg(long)]
        no_cache: bool,

        /// Number of parallel analyses. 0 = auto (CPU count). Uses 4 if not specified.
        #[arg(long, short = 'j')]
        jobs: Option<usize>,
    },

    /// Export results to Excel
    Export {
        /// Path to JSON results file
        results: PathBuf,

        /// Output Excel file path
        #[arg(long, short = 'o')]
        output: Option<PathBuf>,
    },

    /// Manage configuration
    Config {
        /// Show current configuration
        #[arg(long)]
        show: bool,

        /// Set backend
        #[arg(long)]
        set_backend: Option<String>,

        /// Set model
        #[arg(long)]
        set_model: Option<String>,

        /// Enable/disable cache
        #[arg(long)]
        set_cache: Option<bool>,

        /// Set default output format
        #[arg(long)]
        set_output: Option<OutputFormat>,

        /// Set default ensemble count
        #[arg(long)]
        set_ensemble: Option<u32>,

        /// Reset to defaults
        #[arg(long)]
        reset: bool,
    },

    /// Manage cache
    Cache {
        /// Clear all cache
        #[arg(long)]
        clear: bool,

        /// Show cache statistics
        #[arg(long)]
        stats: bool,
    },
}
