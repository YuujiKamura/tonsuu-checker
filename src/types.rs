//! Core types for tonnage estimation

#![allow(dead_code)]

use serde::{Deserialize, Serialize};

/// Material breakdown in mixed loads
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct MaterialBreakdown {
    pub material: String,
    pub percentage: f64,
    pub density: f64,
}

/// AI estimation result from image analysis
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct EstimationResult {
    /// Whether a target (dump truck with cargo) was detected
    pub is_target_detected: bool,

    /// Truck type: "2t", "4t", "増トン", "10t"
    pub truck_type: String,

    /// License plate info (if detected)
    #[serde(default)]
    pub license_plate: Option<String>,

    /// License number (if detected)
    #[serde(default)]
    pub license_number: Option<String>,

    /// Material type: "土砂", "As殻", "Co殻", "開粒度As殻"
    pub material_type: String,

    /// Estimated volume in cubic meters
    pub estimated_volume_m3: f64,

    /// Estimated weight in tonnes
    pub estimated_tonnage: f64,

    /// AI's visual estimate of max capacity
    #[serde(default)]
    pub estimated_max_capacity: Option<f64>,

    /// Confidence score (0.0 - 1.0)
    pub confidence_score: f64,

    /// Reasoning / calculation description
    pub reasoning: String,

    /// Material breakdown for mixed loads
    #[serde(default)]
    pub material_breakdown: Vec<MaterialBreakdown>,

    /// Number of ensemble samples used
    #[serde(default)]
    pub ensemble_count: Option<u32>,
}

impl Default for EstimationResult {
    fn default() -> Self {
        Self {
            is_target_detected: false,
            truck_type: String::new(),
            license_plate: None,
            license_number: None,
            material_type: String::new(),
            estimated_volume_m3: 0.0,
            estimated_tonnage: 0.0,
            estimated_max_capacity: None,
            confidence_score: 0.0,
            reasoning: String::new(),
            material_breakdown: Vec::new(),
            ensemble_count: None,
        }
    }
}

/// Truck specification
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct TruckSpec {
    /// Display name
    pub name: String,
    /// Maximum payload capacity in tonnes
    pub max_capacity: f64,
    /// Cargo bed length in meters
    pub bed_length: f64,
    /// Cargo bed width in meters
    pub bed_width: f64,
    /// Cargo bed height (side wall) in meters
    pub bed_height: f64,
    /// Level (flush) volume in m³
    pub level_volume: f64,
    /// Heaped volume in m³
    pub heap_volume: f64,
}

/// Material properties
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct MaterialSpec {
    /// Display name
    pub name: String,
    /// Density in t/m³
    pub density: f64,
    /// Void ratio (0.0 - 1.0)
    pub void_ratio: f64,
}

/// Load grade classification
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub enum LoadGrade {
    /// Too light (0-80%)
    TooLight,
    /// Light (80-90%)
    Light,
    /// Just right (90-95%)
    JustRight,
    /// Marginal (95-100%)
    Marginal,
    /// Overloaded (>100%)
    Overloaded,
}

impl LoadGrade {
    pub fn from_ratio(ratio: f64) -> Self {
        match ratio {
            r if r < 0.80 => LoadGrade::TooLight,
            r if r < 0.90 => LoadGrade::Light,
            r if r < 0.95 => LoadGrade::JustRight,
            r if r <= 1.00 => LoadGrade::Marginal,
            _ => LoadGrade::Overloaded,
        }
    }

    pub fn label(&self) -> &'static str {
        match self {
            LoadGrade::TooLight => "軽すぎ",
            LoadGrade::Light => "軽め",
            LoadGrade::JustRight => "ちょうど",
            LoadGrade::Marginal => "ギリOK",
            LoadGrade::Overloaded => "積みすぎ",
        }
    }

    pub fn label_en(&self) -> &'static str {
        match self {
            LoadGrade::TooLight => "too_light",
            LoadGrade::Light => "light",
            LoadGrade::JustRight => "just_right",
            LoadGrade::Marginal => "marginal",
            LoadGrade::Overloaded => "overloaded",
        }
    }
}

/// Analysis result with metadata
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct AnalysisEntry {
    /// Image file path
    pub image_path: String,
    /// Analysis timestamp
    pub timestamp: chrono::DateTime<chrono::Utc>,
    /// Estimation result
    pub result: EstimationResult,
    /// Load grade
    pub grade: Option<LoadGrade>,
    /// Actual tonnage (if known)
    pub actual_tonnage: Option<f64>,
}

/// Batch analysis results
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct BatchResults {
    /// Analysis entries
    pub entries: Vec<AnalysisEntry>,
    /// Total images processed
    pub total_processed: usize,
    /// Number of successful analyses
    pub successful: usize,
    /// Number of failed analyses
    pub failed: usize,
    /// Analysis start time
    pub started_at: chrono::DateTime<chrono::Utc>,
    /// Analysis end time
    pub completed_at: chrono::DateTime<chrono::Utc>,
}
