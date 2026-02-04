//! Application Layer
//!
//! This module provides the application layer that orchestrates between
//! the UI (CLI/GUI) and the domain/infrastructure layers.
//!
//! The app layer contains:
//! - `analysis_service`: Core use case for analyzing truck images
//! - `query_service`: Query stored data (history, vehicles)

pub mod analysis_service;
pub mod query_service;

// Re-export main types for convenience
pub use analysis_service::{
    analyze_truck_image, AnalysisOptions, AnalysisServiceError,
};
