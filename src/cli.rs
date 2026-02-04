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
    #[arg(long, short = 'f', global = true)]
    pub format: Option<OutputFormat>,

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

        /// Specify license plate for vehicle matching (e.g., "熊本 130 ら 1122")
        #[arg(long, short = 'p')]
        plate: Option<String>,

        /// Skip YOLO plate detection, use class only (2t, 4t, 増トン, 10t)
        #[arg(long)]
        skip_yolo_class_only: Option<String>,

        /// Filter by transport company name (e.g., "松尾運搬")
        #[arg(long)]
        company: Option<String>,

        /// Material type pre-info (e.g., "As殻", "Co殻", "土砂")
        #[arg(long)]
        material: Option<String>,

        /// Truck class pre-info (e.g., "4tダンプ", "10tダンプ")
        #[arg(long)]
        truck_class: Option<String>,
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

        /// Enable/disable local license plate detection
        #[arg(long)]
        set_plate_local: Option<bool>,

        /// Set local plate detection command
        #[arg(long)]
        set_plate_local_cmd: Option<String>,

        /// Set local plate detection minimum confidence (0.0-1.0)
        #[arg(long)]
        set_plate_local_min_conf: Option<f32>,

        /// If local detection fails, fall back to API stage1
        #[arg(long)]
        set_plate_local_fallback: Option<bool>,

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

    /// Add ground truth feedback for an analyzed image
    Feedback {
        /// Path to image file
        image: PathBuf,

        /// Actual tonnage (ground truth)
        #[arg(long, short = 't')]
        actual: f64,

        /// Optional notes
        #[arg(long, short = 'n')]
        notes: Option<String>,
    },

    /// Show analysis history
    History {
        /// Show only entries with feedback
        #[arg(long)]
        with_feedback: bool,

        /// Limit number of entries shown
        #[arg(long, short = 'n', default_value = "20")]
        limit: usize,
    },

    /// Show accuracy statistics
    Accuracy {
        /// Group by truck type
        #[arg(long)]
        by_truck: bool,

        /// Group by material type
        #[arg(long)]
        by_material: bool,

        /// Show detailed per-sample breakdown
        #[arg(long)]
        detailed: bool,
    },

    /// Auto-collect vehicles from folder (scan 車検証 PDFs and photos)
    AutoCollect {
        /// Path to folder containing vehicle subfolders
        folder: PathBuf,

        /// Skip confirmation prompt
        #[arg(long, short = 'y')]
        yes: bool,

        /// Number of parallel analyses (default: 1)
        #[arg(long, short = 'j', default_value = "1")]
        jobs: usize,

        /// Dry run - scan only, don't register
        #[arg(long)]
        dry_run: bool,

        /// Transport company name (e.g., "松尾運搬")
        #[arg(long, short = 'c')]
        company: Option<String>,
    },

    /// Import backup data from TonSuuChecker app
    Import {
        /// Path to backup JSON file
        file: PathBuf,

        /// Dry run - show what would be imported without actually importing
        #[arg(long)]
        dry_run: bool,
    },

    /// Check AI backend status and rate limits
    Stats,

    /// Check for overloaded vehicles by comparing weighing slips with vehicle master
    CheckOverload {
        /// Path to CSV file containing weighing slips
        #[arg(long)]
        csv: PathBuf,

        /// Path to CSV file containing vehicle master data
        #[arg(long)]
        vehicles: PathBuf,

        /// Output format (json for machine-readable, table for human-readable)
        #[arg(long, short = 'o')]
        output: Option<OutputFormat>,
    },
}
