//! Persistent store for analysis results with ground truth

pub mod vehicles;

pub use vehicles::VehicleStore;

use crate::error::{CacheError, Result};
use crate::types::{EstimationResult, LoadGrade, TruckClass};
use chrono::{DateTime, Utc};
use serde::{Deserialize, Serialize};
use sha2::{Digest, Sha256};
use std::collections::HashMap;
use std::fs::{self, File};
use std::io::{BufReader, BufWriter};
use std::path::{Path, PathBuf};

/// Entry in the history store
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct HistoryEntry {
    /// Image file path (original)
    pub image_path: String,

    /// SHA256 hash of the image (used as key)
    pub image_hash: String,

    /// AI estimation result
    pub estimation: EstimationResult,

    /// Ground truth tonnage (if provided)
    #[serde(default)]
    pub actual_tonnage: Option<f64>,

    /// Maximum capacity from vehicle registration (車検証)
    #[serde(default)]
    pub max_capacity: Option<f64>,

    /// When the analysis was performed
    pub analyzed_at: DateTime<Utc>,

    /// When ground truth was added (if any)
    #[serde(default)]
    pub feedback_at: Option<DateTime<Utc>>,

    /// Optional notes
    #[serde(default)]
    pub notes: Option<String>,

    /// Base64 encoded thumbnail for reference (optional)
    #[serde(default)]
    pub thumbnail_base64: Option<String>,
}

/// History entry with load grade information for staged analysis
#[derive(Debug, Clone)]
pub struct GradedHistoryEntry {
    /// Original history entry
    pub entry: HistoryEntry,
    /// Load grade
    pub grade: LoadGrade,
    /// Load ratio (actual / max_capacity) as percentage
    pub load_ratio: f64,
}

/// Persistent store for history entries
pub struct Store {
    store_path: PathBuf,
    entries: HashMap<String, HistoryEntry>,
}

impl Store {
    /// Create or load a store
    pub fn open(store_dir: PathBuf) -> Result<Self> {
        fs::create_dir_all(&store_dir)?;
        let store_path = store_dir.join("history.json");

        let entries = if store_path.exists() {
            let file = File::open(&store_path)?;
            let reader = BufReader::new(file);
            serde_json::from_reader(reader).unwrap_or_default()
        } else {
            HashMap::new()
        };

        Ok(Self { store_path, entries })
    }

    /// Compute hash for an image file
    pub fn hash_image(image_path: &Path) -> Result<String> {
        let file = File::open(image_path)?;
        let mut reader = BufReader::new(file);
        let mut hasher = Sha256::new();
        std::io::copy(&mut reader, &mut hasher)?;
        let hash = hasher.finalize();
        Ok(format!("{:x}", hash))
    }

    /// Save store to disk
    fn save(&self) -> Result<()> {
        let file = File::create(&self.store_path)?;
        let writer = BufWriter::new(file);
        serde_json::to_writer_pretty(writer, &self.entries)?;
        Ok(())
    }

    /// Add or update an analysis result
    pub fn add_analysis(
        &mut self,
        image_path: &Path,
        estimation: EstimationResult,
    ) -> Result<String> {
        self.add_analysis_with_capacity(image_path, estimation, None, None)
    }

    /// Add analysis result with optional max capacity and thumbnail
    pub fn add_analysis_with_capacity(
        &mut self,
        image_path: &Path,
        estimation: EstimationResult,
        max_capacity: Option<f64>,
        thumbnail_base64: Option<String>,
    ) -> Result<String> {
        let hash = Self::hash_image(image_path)?;

        let entry = HistoryEntry {
            image_path: image_path.display().to_string(),
            image_hash: hash.clone(),
            estimation,
            actual_tonnage: None,
            max_capacity,
            analyzed_at: Utc::now(),
            feedback_at: None,
            notes: None,
            thumbnail_base64,
        };

        self.entries.insert(hash.clone(), entry);
        self.save()?;
        Ok(hash)
    }

    /// Add ground truth feedback for an image
    pub fn add_feedback(
        &mut self,
        image_path: &Path,
        actual_tonnage: f64,
        notes: Option<String>,
    ) -> Result<()> {
        self.add_feedback_with_capacity(image_path, actual_tonnage, None, notes)
    }

    /// Add ground truth feedback with optional max capacity
    pub fn add_feedback_with_capacity(
        &mut self,
        image_path: &Path,
        actual_tonnage: f64,
        max_capacity: Option<f64>,
        notes: Option<String>,
    ) -> Result<()> {
        let hash = Self::hash_image(image_path)?;

        if let Some(entry) = self.entries.get_mut(&hash) {
            entry.actual_tonnage = Some(actual_tonnage);
            entry.feedback_at = Some(Utc::now());
            if let Some(cap) = max_capacity {
                entry.max_capacity = Some(cap);
            }
            if notes.is_some() {
                entry.notes = notes;
            }
            self.save()?;
            Ok(())
        } else {
            Err(CacheError::IoError(format!(
                "No analysis found for image: {}",
                image_path.display()
            ))
            .into())
        }
    }

    /// Get entries with both actual_tonnage and max_capacity (judged items)
    pub fn get_judged_items(&self) -> Vec<&HistoryEntry> {
        self.entries
            .values()
            .filter(|e| e.actual_tonnage.is_some() && e.max_capacity.is_some())
            .collect()
    }

    /// Select graded stock items by truck class
    /// Returns one representative item per load grade for the given truck class
    pub fn select_stock_by_grade(&self, target_class: TruckClass) -> Vec<GradedHistoryEntry> {
        let judged_items = self.get_judged_items();

        // Filter by same truck class
        let same_class_items: Vec<_> = judged_items
            .into_iter()
            .filter(|entry| {
                entry
                    .max_capacity
                    .map(|cap| TruckClass::from_capacity(cap) == target_class)
                    .unwrap_or(false)
            })
            .collect();

        // Add grade information to each item
        let graded_items: Vec<GradedHistoryEntry> = same_class_items
            .into_iter()
            .filter_map(|entry| {
                let actual = entry.actual_tonnage?;
                let max_cap = entry.max_capacity?;
                let load_ratio = (actual / max_cap) * 100.0;
                let grade = LoadGrade::from_ratio(actual / max_cap);
                Some(GradedHistoryEntry {
                    entry: entry.clone(),
                    grade,
                    load_ratio,
                })
            })
            .collect();

        // Select the latest 1 item per grade
        let grades = [
            LoadGrade::TooLight,
            LoadGrade::Light,
            LoadGrade::JustRight,
            LoadGrade::Marginal,
            LoadGrade::Overloaded,
        ];

        let mut result = Vec::new();
        for grade in grades {
            let mut items_in_grade: Vec<_> = graded_items
                .iter()
                .filter(|item| item.grade == grade)
                .cloned()
                .collect();

            // Sort by timestamp descending (newest first)
            items_in_grade.sort_by(|a, b| b.entry.analyzed_at.cmp(&a.entry.analyzed_at));

            if let Some(latest) = items_in_grade.first() {
                result.push(latest.clone());
            }
        }

        result
    }

    /// Get entry by image path
    pub fn get_by_path(&self, image_path: &Path) -> Result<Option<&HistoryEntry>> {
        let hash = Self::hash_image(image_path)?;
        Ok(self.entries.get(&hash))
    }

    /// Get entry by hash
    pub fn get_by_hash(&self, hash: &str) -> Option<&HistoryEntry> {
        self.entries.get(hash)
    }

    /// Get all entries
    pub fn all_entries(&self) -> Vec<&HistoryEntry> {
        let mut entries: Vec<_> = self.entries.values().collect();
        entries.sort_by(|a, b| b.analyzed_at.cmp(&a.analyzed_at));
        entries
    }

    /// Get entries with ground truth
    pub fn entries_with_feedback(&self) -> Vec<&HistoryEntry> {
        self.all_entries()
            .into_iter()
            .filter(|e| e.actual_tonnage.is_some())
            .collect()
    }

    /// Get total entry count
    pub fn count(&self) -> usize {
        self.entries.len()
    }

    /// Get count of entries with feedback
    pub fn feedback_count(&self) -> usize {
        self.entries
            .values()
            .filter(|e| e.actual_tonnage.is_some())
            .count()
    }

    /// Calculate accuracy statistics
    pub fn accuracy_stats(&self) -> AccuracyStats {
        let entries: Vec<_> = self
            .entries
            .values()
            .filter_map(|e| {
                e.actual_tonnage.map(|actual| AccuracySample {
                    estimated: e.estimation.estimated_tonnage,
                    actual,
                    truck_type: e.estimation.truck_type.clone(),
                    material_type: e.estimation.material_type.clone(),
                })
            })
            .collect();

        AccuracyStats::from_samples(entries)
    }
}

/// Single sample for accuracy calculation
#[derive(Debug, Clone)]
pub struct AccuracySample {
    pub estimated: f64,
    pub actual: f64,
    pub truck_type: String,
    pub material_type: String,
}

impl AccuracySample {
    pub fn error(&self) -> f64 {
        self.estimated - self.actual
    }

    pub fn abs_error(&self) -> f64 {
        self.error().abs()
    }

    pub fn percent_error(&self) -> f64 {
        if self.actual > 0.0 {
            (self.error() / self.actual) * 100.0
        } else {
            0.0
        }
    }
}

/// Accuracy statistics
#[derive(Debug, Clone, Default)]
pub struct AccuracyStats {
    pub sample_count: usize,
    pub mean_error: f64,
    pub mean_abs_error: f64,
    pub mean_percent_error: f64,
    pub rmse: f64,
    pub max_error: f64,
    pub min_error: f64,
    pub samples: Vec<AccuracySample>,
}

impl AccuracyStats {
    pub fn from_samples(samples: Vec<AccuracySample>) -> Self {
        if samples.is_empty() {
            return Self::default();
        }

        let n = samples.len() as f64;

        let sum_error: f64 = samples.iter().map(|s| s.error()).sum();
        let sum_abs_error: f64 = samples.iter().map(|s| s.abs_error()).sum();
        let sum_pct_error: f64 = samples.iter().map(|s| s.percent_error().abs()).sum();
        let sum_sq_error: f64 = samples.iter().map(|s| s.error().powi(2)).sum();

        let errors: Vec<f64> = samples.iter().map(|s| s.error()).collect();
        let max_error = errors.iter().cloned().fold(f64::NEG_INFINITY, f64::max);
        let min_error = errors.iter().cloned().fold(f64::INFINITY, f64::min);

        Self {
            sample_count: samples.len(),
            mean_error: sum_error / n,
            mean_abs_error: sum_abs_error / n,
            mean_percent_error: sum_pct_error / n,
            rmse: (sum_sq_error / n).sqrt(),
            max_error,
            min_error,
            samples,
        }
    }

    /// Group by truck type
    pub fn by_truck_type(&self) -> HashMap<String, AccuracyStats> {
        let mut groups: HashMap<String, Vec<AccuracySample>> = HashMap::new();
        for sample in &self.samples {
            groups
                .entry(sample.truck_type.clone())
                .or_default()
                .push(sample.clone());
        }
        groups
            .into_iter()
            .map(|(k, v)| (k, Self::from_samples(v)))
            .collect()
    }

    /// Group by material type
    pub fn by_material_type(&self) -> HashMap<String, AccuracyStats> {
        let mut groups: HashMap<String, Vec<AccuracySample>> = HashMap::new();
        for sample in &self.samples {
            groups
                .entry(sample.material_type.clone())
                .or_default()
                .push(sample.clone());
        }
        groups
            .into_iter()
            .map(|(k, v)| (k, Self::from_samples(v)))
            .collect()
    }
}
