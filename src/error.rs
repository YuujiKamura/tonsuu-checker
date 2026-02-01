//! Error types for tonnage-checker

#![allow(dead_code)]

use thiserror::Error;

/// Configuration-related errors
#[derive(Debug, Error)]
pub enum ConfigError {
    #[error("Configuration not found")]
    NotFound,

    #[error("Failed to parse configuration: {0}")]
    ParseError(String),

    #[error("Failed to save configuration: {0}")]
    SaveError(String),
}

/// Cache-related errors
#[derive(Debug, Error)]
pub enum CacheError {
    #[error("Cache entry not found")]
    NotFound,

    #[error("Cache data corrupted: {0}")]
    Corrupted(String),

    #[error("Cache IO error: {0}")]
    IoError(String),
}

#[derive(Debug, Error)]
pub enum Error {
    #[error("IO error: {0}")]
    Io(#[from] std::io::Error),

    #[error("Image processing error: {0}")]
    Image(#[from] image::ImageError),

    #[error("JSON error: {0}")]
    Json(#[from] serde_json::Error),

    #[error("AI analyzer error: {0}")]
    Analyzer(#[from] cli_ai_analyzer::Error),

    #[error("Configuration error: {0}")]
    Config(#[from] ConfigError),

    #[error("Cache error: {0}")]
    Cache(#[from] CacheError),

    #[error("File not found: {0}")]
    FileNotFound(String),

    #[error("Invalid image format: {0}")]
    InvalidImageFormat(String),

    #[error("Analysis failed: {0}")]
    AnalysisFailed(String),

    #[error("Excel export error: {0}")]
    Excel(String),

    #[error("No target detected in image")]
    NoTargetDetected,
}

pub type Result<T> = std::result::Result<T, Error>;
