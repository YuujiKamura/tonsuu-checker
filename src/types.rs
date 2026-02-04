//! Core types for tonnage estimation

#![allow(dead_code)]

use serde::{Deserialize, Deserializer, Serialize};

/// Deserialize null as default value
fn null_to_default<'de, D, T>(deserializer: D) -> Result<T, D::Error>
where
    D: Deserializer<'de>,
    T: Default + Deserialize<'de>,
{
    Option::deserialize(deserializer).map(|opt| opt.unwrap_or_default())
}

/// Truck class based on max capacity
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
pub enum TruckClass {
    /// 2t class (1.5-2.5t)
    TwoTon,
    /// 4t class (3.0-4.5t)
    FourTon,
    /// 増トン (5.0-8.0t)
    IncreasedTon,
    /// 10t class (9.0-12.0t)
    TenTon,
    /// Unknown
    Unknown,
}

impl TruckClass {
    /// Determine truck class from max capacity
    pub fn from_capacity(max_capacity: f64) -> Self {
        match max_capacity {
            c if c >= 1.5 && c <= 2.5 => TruckClass::TwoTon,
            c if c >= 3.0 && c <= 4.5 => TruckClass::FourTon,
            c if c >= 5.0 && c <= 8.0 => TruckClass::IncreasedTon,
            c if c >= 9.0 && c <= 12.0 => TruckClass::TenTon,
            _ => TruckClass::Unknown,
        }
    }

    /// Get display label in Japanese
    pub fn label(&self) -> &'static str {
        match self {
            TruckClass::TwoTon => "2t",
            TruckClass::FourTon => "4t",
            TruckClass::IncreasedTon => "増トン",
            TruckClass::TenTon => "10t",
            TruckClass::Unknown => "不明",
        }
    }
}

/// Registered vehicle information
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct RegisteredVehicle {
    /// Unique identifier
    pub id: String,
    /// Vehicle name (e.g., "日野 プロフィア", "いすゞ ギガ")
    pub name: String,
    /// Maximum payload capacity in tonnes
    pub max_capacity: f64,
    /// License plate number (optional)
    #[serde(default)]
    pub license_plate: Option<String>,
    /// Transport company name (e.g., "松尾運搬")
    #[serde(default)]
    pub company: Option<String>,
    /// Vehicle image path
    #[serde(default)]
    pub image_path: Option<String>,
    /// Thumbnail as base64 for AI reference
    #[serde(default)]
    pub thumbnail_base64: Option<String>,
    /// Notes/memo
    #[serde(default)]
    pub notes: Option<String>,
    /// When registered
    pub registered_at: chrono::DateTime<chrono::Utc>,
}

impl RegisteredVehicle {
    pub fn new(name: String, max_capacity: f64) -> Self {
        Self {
            id: uuid::Uuid::new_v4().to_string(),
            name,
            max_capacity,
            license_plate: None,
            company: None,
            image_path: None,
            thumbnail_base64: None,
            notes: None,
            registered_at: chrono::Utc::now(),
        }
    }

    pub fn with_image(mut self, image_path: String, thumbnail_base64: Option<String>) -> Self {
        self.image_path = Some(image_path);
        self.thumbnail_base64 = thumbnail_base64;
        self
    }

    pub fn with_license_plate(mut self, plate: String) -> Self {
        self.license_plate = Some(plate);
        self
    }

    pub fn truck_class(&self) -> TruckClass {
        TruckClass::from_capacity(self.max_capacity)
    }
}

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
    #[serde(default, deserialize_with = "null_to_default")]
    pub is_target_detected: bool,

    /// Truck type: "2t", "4t", "増トン", "10t"
    #[serde(default, deserialize_with = "null_to_default")]
    pub truck_type: String,

    /// License plate info (if detected)
    #[serde(default)]
    pub license_plate: Option<String>,

    /// License number (if detected)
    #[serde(default)]
    pub license_number: Option<String>,

    /// Material type: "土砂", "As殻", "Co殻", "開粒度As殻"
    #[serde(default, deserialize_with = "null_to_default")]
    pub material_type: String,

    /// 上面積 (m²)
    #[serde(default, alias = "upperArea")]
    pub upper_area: Option<f64>,

    /// 高さ (m)
    #[serde(default)]
    pub height: Option<f64>,

    /// せん断変形角度 (度)
    #[serde(default)]
    pub slope: Option<f64>,

    /// Void ratio (0.30-0.40 for rubble)
    #[serde(default)]
    pub void_ratio: Option<f64>,

    /// Estimated volume in cubic meters
    #[serde(default, deserialize_with = "null_to_default")]
    pub estimated_volume_m3: f64,

    /// Estimated weight in tonnes
    #[serde(default, deserialize_with = "null_to_default")]
    pub estimated_tonnage: f64,

    /// Confidence score (0.0 - 1.0)
    #[serde(default, deserialize_with = "null_to_default")]
    pub confidence_score: f64,

    /// Reasoning / calculation description
    #[serde(default, deserialize_with = "null_to_default")]
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
            upper_area: None,
            height: None,
            slope: None,
            void_ratio: None,
            estimated_volume_m3: 0.0,
            estimated_tonnage: 0.0,
            confidence_score: 0.0,
            reasoning: String::new(),
            material_breakdown: Vec::new(),
            ensemble_count: None,
        }
    }
}

// Re-export domain types for backwards compatibility

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
