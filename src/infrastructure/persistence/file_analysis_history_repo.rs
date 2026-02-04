//! File-based analysis history repository implementation
//!
//! Note: Alternative history storage implementation using domain repository pattern.
//! Currently unused but maintained for future repository abstraction.

#![allow(dead_code)]

use crate::domain::AnalysisHistoryRepository;
use crate::error::{CacheError, Error, Result};
use crate::store::{AccuracySample, AccuracyStats, GradedHistoryEntry, HistoryEntry};
use crate::types::{EstimationResult, LoadGrade, TruckClass};
use chrono::Utc;
use sha2::{Digest, Sha256};
use std::cell::RefCell;
use std::collections::HashMap;
use std::fs::{self, File};
use std::io::{BufReader, BufWriter};
use std::path::{Path, PathBuf};

/// File-based implementation of AnalysisHistoryRepository
///
/// Stores analysis history entries in a JSON file on disk.
pub struct FileAnalysisHistoryRepository {
    store_path: PathBuf,
    entries: RefCell<HashMap<String, HistoryEntry>>,
}

impl FileAnalysisHistoryRepository {
    /// Create or load a history repository
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

        Ok(Self {
            store_path,
            entries: RefCell::new(entries),
        })
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
    fn persist(&self) -> Result<()> {
        let file = File::create(&self.store_path)?;
        let writer = BufWriter::new(file);
        serde_json::to_writer_pretty(writer, &*self.entries.borrow())?;
        Ok(())
    }

    /// Add or update an analysis result
    pub fn add_analysis(&self, image_path: &Path, estimation: EstimationResult) -> Result<String> {
        self.add_analysis_with_capacity(image_path, estimation, None, None)
    }

    /// Add analysis result with optional max capacity and thumbnail
    pub fn add_analysis_with_capacity(
        &self,
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

        self.entries.borrow_mut().insert(hash.clone(), entry);
        self.persist()?;
        Ok(hash)
    }

    /// Add ground truth feedback for an image
    pub fn add_feedback(
        &self,
        image_path: &Path,
        actual_tonnage: f64,
        notes: Option<String>,
    ) -> Result<()> {
        self.add_feedback_with_capacity(image_path, actual_tonnage, None, notes)
    }

    /// Add ground truth feedback with optional max capacity
    pub fn add_feedback_with_capacity(
        &self,
        image_path: &Path,
        actual_tonnage: f64,
        max_capacity: Option<f64>,
        notes: Option<String>,
    ) -> Result<()> {
        let hash = Self::hash_image(image_path)?;

        let mut entries = self.entries.borrow_mut();
        if let Some(entry) = entries.get_mut(&hash) {
            entry.actual_tonnage = Some(actual_tonnage);
            entry.feedback_at = Some(Utc::now());
            if let Some(cap) = max_capacity {
                entry.max_capacity = Some(cap);
            }
            if notes.is_some() {
                entry.notes = notes;
            }
            drop(entries);
            self.persist()?;
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
    pub fn get_judged_items(&self) -> Vec<HistoryEntry> {
        self.entries
            .borrow()
            .values()
            .filter(|e| e.actual_tonnage.is_some() && e.max_capacity.is_some())
            .cloned()
            .collect()
    }

    /// Select graded stock items by truck class
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
                    entry,
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
    pub fn get_by_path(&self, image_path: &Path) -> Result<Option<HistoryEntry>> {
        let hash = Self::hash_image(image_path)?;
        Ok(self.entries.borrow().get(&hash).cloned())
    }

    /// Add a pre-built history entry (for import)
    pub fn add_entry(&self, entry: HistoryEntry) -> Result<bool> {
        let mut entries = self.entries.borrow_mut();
        if entries.contains_key(&entry.image_hash) {
            return Ok(false);
        }
        entries.insert(entry.image_hash.clone(), entry);
        drop(entries);
        self.persist()?;
        Ok(true)
    }

    /// Check if entry with given hash exists
    pub fn has_entry(&self, hash: &str) -> bool {
        self.entries.borrow().contains_key(hash)
    }

    /// Get entry by hash
    pub fn get_by_hash(&self, hash: &str) -> Option<HistoryEntry> {
        self.entries.borrow().get(hash).cloned()
    }

    /// Get all entries sorted by timestamp (newest first)
    pub fn all_entries(&self) -> Vec<HistoryEntry> {
        let mut entries: Vec<_> = self.entries.borrow().values().cloned().collect();
        entries.sort_by(|a, b| b.analyzed_at.cmp(&a.analyzed_at));
        entries
    }

    /// Get entries with ground truth
    pub fn entries_with_feedback(&self) -> Vec<HistoryEntry> {
        self.all_entries()
            .into_iter()
            .filter(|e| e.actual_tonnage.is_some())
            .collect()
    }

    /// Get total entry count
    pub fn count(&self) -> usize {
        self.entries.borrow().len()
    }

    /// Get count of entries with feedback
    pub fn feedback_count(&self) -> usize {
        self.entries
            .borrow()
            .values()
            .filter(|e| e.actual_tonnage.is_some())
            .count()
    }

    /// Calculate accuracy statistics
    pub fn accuracy_stats(&self) -> AccuracyStats {
        let entries: Vec<_> = self
            .entries
            .borrow()
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

impl AnalysisHistoryRepository for FileAnalysisHistoryRepository {
    fn save(&self, result: &HistoryEntry) -> std::result::Result<(), Error> {
        self.entries
            .borrow_mut()
            .insert(result.image_hash.clone(), result.clone());
        self.persist()
    }

    fn find_by_id(&self, id: &str) -> std::result::Result<Option<HistoryEntry>, Error> {
        Ok(self.entries.borrow().get(id).cloned())
    }

    fn find_all(&self) -> std::result::Result<Vec<HistoryEntry>, Error> {
        Ok(self.all_entries())
    }
}
