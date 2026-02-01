//! Error types for tonnage-checker

#![allow(dead_code)]

use thiserror::Error;

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

    #[error("Config error: {0}")]
    Config(String),

    #[error("File not found: {0}")]
    FileNotFound(String),

    #[error("Invalid image format: {0}")]
    InvalidImageFormat(String),

    #[error("Analysis failed: {0}")]
    AnalysisFailed(String),

    #[error("Cache error: {0}")]
    Cache(String),

    #[error("Excel export error: {0}")]
    Excel(String),

    #[error("No target detected in image")]
    NoTargetDetected,
}

pub type Result<T> = std::result::Result<T, Error>;
