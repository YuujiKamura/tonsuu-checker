//! Import data from legacy TonSuuChecker_local (TypeScript/React version)
//!
//! Reads the JSON backup format exported by the old web app.
//!
//! Note: This module imports data from the previous TypeScript version.
//! Currently unused but maintained for migration support.

#![allow(dead_code)]

use std::collections::HashMap;
use std::fs;
use std::path::Path;

use chrono::{TimeZone, Utc};
use serde::{Deserialize, Serialize};

use crate::error::{Error, Result};
use crate::store::HistoryEntry;
use crate::types::EstimationResult;

/// Legacy export data format from TonSuuChecker_local
#[allow(dead_code)]
#[derive(Debug, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct LegacyExportData {
    pub version: u32,
    pub exported_at: String,
    pub app_name: String,
    pub includes_images: bool,
    pub stock: Vec<LegacyStockItem>,
    #[serde(default)]
    pub vehicles: Vec<LegacyVehicle>,
    #[serde(default)]
    pub chat_history: Option<HashMap<String, Vec<LegacyChatMessage>>>,
    #[serde(default)]
    pub cost_history: Option<Vec<LegacyCostEntry>>,
}

/// Legacy stock item (案件データ)
#[allow(dead_code)]
#[derive(Debug, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct LegacyStockItem {
    pub id: String,
    pub timestamp: i64, // milliseconds
    #[serde(default)]
    pub base64_images: Vec<String>,
    #[serde(default)]
    pub image_urls: Vec<String>,
    pub actual_tonnage: Option<f64>,
    pub max_capacity: Option<f64>,
    pub memo: Option<String>,
    pub manifest_number: Option<String>,
    pub waste_type: Option<String>,
    pub destination: Option<String>,
    pub result: Option<LegacyEstimationResult>,
    #[serde(default)]
    pub estimations: Vec<LegacyEstimationResult>,
    #[serde(default)]
    pub chat_history: Vec<LegacyChatMessage>,
}

/// Legacy estimation result
#[allow(dead_code)]
#[derive(Debug, Clone, Deserialize, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct LegacyEstimationResult {
    pub is_target_detected: bool,
    pub truck_type: String,
    pub license_plate: Option<String>,
    pub license_number: Option<String>,
    pub material_type: String,
    pub estimated_volume_m3: Option<f64>,
    pub estimated_tonnage: Option<f64>,
    pub estimated_max_capacity: Option<f64>,
    pub confidence_score: Option<f64>,
    pub reasoning: Option<String>,
    pub ensemble_count: Option<u32>,
    #[serde(default)]
    pub material_breakdown: Vec<LegacyMaterialBreakdown>,
}

#[allow(dead_code)]
#[derive(Debug, Clone, Deserialize, Serialize)]
pub struct LegacyMaterialBreakdown {
    pub material: String,
    pub percentage: f64,
    pub density: f64,
}

/// Legacy registered vehicle
#[allow(dead_code)]
#[derive(Debug, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct LegacyVehicle {
    pub id: String,
    pub name: String,
    pub max_capacity: f64,
    pub truck_class: Option<String>,
    pub base64: Option<String>,
}

/// Legacy chat message
#[derive(Debug, Clone, Deserialize, Serialize)]
pub struct LegacyChatMessage {
    pub role: String,
    pub content: String,
}

/// Legacy cost entry
#[derive(Debug, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct LegacyCostEntry {
    pub id: String,
    pub timestamp: i64,
    pub model: String,
    pub call_count: u32,
    pub estimated_cost: f64,
    pub image_count: u32,
}

/// Import mode for legacy data import
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub enum ImportMode {
    /// Append mode: Keep existing data and only add new data
    #[default]
    Append,
    /// Refresh mode: Clear existing data before importing all data
    Refresh,
}

/// Import result
#[derive(Debug, Default)]
pub struct ImportResult {
    pub history_imported: usize,
    pub vehicles_imported: usize,
    pub skipped: usize,
    pub cleared: usize,
    pub errors: Vec<String>,
}

impl ImportResult {
    pub fn is_success(&self) -> bool {
        self.errors.is_empty()
    }
}

/// Load legacy export data from JSON file
pub fn load_legacy_export(path: &Path) -> Result<LegacyExportData> {
    let content = fs::read_to_string(path).map_err(|e| {
        Error::FileNotFound(format!("Failed to read legacy export file: {}", e))
    })?;

    serde_json::from_str(&content).map_err(|e| {
        Error::AnalysisFailed(format!("Failed to parse legacy export JSON: {}", e))
    })
}

/// Convert legacy stock item to new HistoryEntry format
pub fn convert_to_history_entry(item: &LegacyStockItem) -> HistoryEntry {
    // Convert timestamp (milliseconds) to DateTime
    let analyzed_at = Utc.timestamp_millis_opt(item.timestamp).single()
        .unwrap_or_else(Utc::now);

    // Get the latest estimation result
    let estimation = item.result.clone()
        .or_else(|| item.estimations.first().cloned());

    // Build estimation result for new format
    let new_estimation = estimation.map(|est| EstimationResult {
        is_target_detected: est.is_target_detected,
        truck_type: est.truck_type,
        license_plate: est.license_plate,
        license_number: est.license_number,
        material_type: est.material_type,
        upper_area: None,
        height: None,
        front_height: None,
        rear_height: None,
        rear_empty_ratio: None,
        frustum_ratio: None,
        slope: None,
        void_ratio: None,
        estimated_volume_m3: est.estimated_volume_m3.unwrap_or_default(),
        estimated_tonnage: est.estimated_tonnage.unwrap_or_default(),
        confidence_score: est.confidence_score.unwrap_or_default(),
        reasoning: est.reasoning.unwrap_or_default(),
        material_breakdown: est.material_breakdown.into_iter().map(|mb| {
            crate::types::MaterialBreakdown {
                material: mb.material,
                percentage: mb.percentage,
                density: mb.density,
            }
        }).collect(),
        ensemble_count: est.ensemble_count,
    }).unwrap_or_default();

    // Create image path placeholder
    let image_path = format!("[imported from backup: {}]", item.id);

    HistoryEntry {
        image_path,
        image_hash: item.id.clone(),
        estimation: new_estimation,
        actual_tonnage: item.actual_tonnage,
        max_capacity: item.max_capacity,
        analyzed_at,
        feedback_at: None,
        notes: item.memo.clone(),
        thumbnail_base64: item.base64_images.first().cloned(),
    }
}

/// Import all data from legacy export
///
/// # Arguments
/// * `export_data` - The legacy export data to import
/// * `store` - The store to import data into
/// * `mode` - Import mode (Append or Refresh)
///
/// # Import Modes
/// * `ImportMode::Append` - Keep existing data and only add new entries (skip duplicates)
/// * `ImportMode::Refresh` - Clear all existing data before importing
pub fn import_legacy_data(
    export_data: &LegacyExportData,
    store: &mut crate::store::Store,
    mode: ImportMode,
) -> ImportResult {
    let mut result = ImportResult::default();

    // Handle Refresh mode: clear existing data first
    if mode == ImportMode::Refresh {
        result.cleared = store.count();
        if let Err(e) = store.clear() {
            result.errors.push(format!("Failed to clear existing data: {}", e));
            return result;
        }
    }

    // Import stock items as history entries
    for item in &export_data.stock {
        let entry = convert_to_history_entry(item);

        // In Append mode, check if already exists
        if mode == ImportMode::Append && store.has_entry(&entry.image_hash) {
            result.skipped += 1;
            continue;
        }

        match store.add_entry(entry) {
            Ok(_) => result.history_imported += 1,
            Err(e) => result.errors.push(format!("Failed to import {}: {}", item.id, e)),
        }
    }

    result
}

/// Import data from backup file
///
/// # Arguments
/// * `path` - Path to the backup JSON file
/// * `store` - The store to import data into
/// * `mode` - Import mode (Append or Refresh)
///
/// # Import Modes
/// * `ImportMode::Append` - Keep existing data and only add new entries (skip duplicates)
/// * `ImportMode::Refresh` - Clear all existing data before importing
pub fn import_from_backup(
    path: &Path,
    store: &mut crate::store::Store,
    mode: ImportMode,
) -> Result<ImportResult> {
    let export_data = load_legacy_export(path)?;
    Ok(import_legacy_data(&export_data, store, mode))
}

/// Generate summary report of legacy data
pub fn summarize_legacy_export(data: &LegacyExportData) -> String {
    let mut report = String::new();

    report.push_str(&format!("=== Legacy TonSuuChecker Backup ===\n"));
    report.push_str(&format!("Version: {}\n", data.version));
    report.push_str(&format!("Exported at: {}\n", data.exported_at));
    report.push_str(&format!("Includes images: {}\n", data.includes_images));
    report.push_str(&format!("\n"));
    report.push_str(&format!("Stock items: {} 件\n", data.stock.len()));
    report.push_str(&format!("Vehicles: {} 件\n", data.vehicles.len()));

    if let Some(chat) = &data.chat_history {
        report.push_str(&format!("Chat histories: {} 件\n", chat.len()));
    }

    if let Some(cost) = &data.cost_history {
        report.push_str(&format!("Cost entries: {} 件\n", cost.len()));
    }

    // Show sample data
    if !data.stock.is_empty() {
        report.push_str(&format!("\n=== Sample Stock Items ===\n"));
        for (i, item) in data.stock.iter().take(5).enumerate() {
            let dt = Utc.timestamp_millis_opt(item.timestamp).single()
                .map(|d| d.format("%Y/%m/%d %H:%M").to_string())
                .unwrap_or_else(|| "unknown".to_string());

            let tonnage = item.result.as_ref()
                .and_then(|r| r.estimated_tonnage)
                .map(|t| format!("{:.1}t", t))
                .unwrap_or_else(|| "-".to_string());

            let truck = item.result.as_ref()
                .map(|r| r.truck_type.as_str())
                .unwrap_or("-");

            report.push_str(&format!(
                "{}. {} {} {} {}\n",
                i + 1, dt, tonnage, truck,
                item.waste_type.as_deref().unwrap_or("-")
            ));
        }

        if data.stock.len() > 5 {
            report.push_str(&format!("... and {} more\n", data.stock.len() - 5));
        }
    }

    report
}

#[cfg(test)]
mod tests {
    use super::*;
    use tempfile::tempdir;

    #[test]
    fn test_parse_legacy_json() {
        let json = r#"{
            "version": 1,
            "exportedAt": "2024-01-15T10:00:00Z",
            "appName": "TonSuuChecker",
            "includesImages": false,
            "stock": [{
                "id": "test-001",
                "timestamp": 1705312800000,
                "base64Images": [],
                "imageUrls": [],
                "result": {
                    "isTargetDetected": true,
                    "truckType": "4t",
                    "materialType": "As殻",
                    "estimatedTonnage": 3.5,
                    "confidenceScore": 0.85,
                    "materialBreakdown": []
                }
            }],
            "vehicles": []
        }"#;

        let data: LegacyExportData = serde_json::from_str(json).unwrap();
        assert_eq!(data.version, 1);
        assert_eq!(data.stock.len(), 1);
        assert_eq!(data.stock[0].result.as_ref().unwrap().truck_type, "4t");
    }

    #[test]
    fn test_import_append_mode() {
        let temp_dir = tempdir().expect("Failed to create temp dir");
        let mut store = crate::store::Store::open(temp_dir.path().to_path_buf())
            .expect("Failed to open store");

        let json = r#"{
            "version": 1,
            "exportedAt": "2024-01-15T10:00:00Z",
            "appName": "TonSuuChecker",
            "includesImages": false,
            "stock": [
                {"id": "item-001", "timestamp": 1705312800000, "base64Images": [], "imageUrls": [],
                 "result": {"isTargetDetected": true, "truckType": "4t", "materialType": "As殻",
                            "estimatedTonnage": 3.5, "confidenceScore": 0.85, "materialBreakdown": []}},
                {"id": "item-002", "timestamp": 1705312900000, "base64Images": [], "imageUrls": [],
                 "result": {"isTargetDetected": true, "truckType": "10t", "materialType": "Co殻",
                            "estimatedTonnage": 8.2, "confidenceScore": 0.90, "materialBreakdown": []}}
            ],
            "vehicles": []
        }"#;

        let data: LegacyExportData = serde_json::from_str(json).unwrap();

        // First import
        let result = import_legacy_data(&data, &mut store, ImportMode::Append);
        assert!(result.is_success());
        assert_eq!(result.history_imported, 2);
        assert_eq!(result.skipped, 0);
        assert_eq!(store.count(), 2);

        // Second import (same data) - should skip duplicates
        let result2 = import_legacy_data(&data, &mut store, ImportMode::Append);
        assert!(result2.is_success());
        assert_eq!(result2.history_imported, 0);
        assert_eq!(result2.skipped, 2);
        assert_eq!(store.count(), 2); // Still 2 items
    }

    #[test]
    fn test_import_refresh_mode() {
        let temp_dir = tempdir().expect("Failed to create temp dir");
        let mut store = crate::store::Store::open(temp_dir.path().to_path_buf())
            .expect("Failed to open store");

        let json1 = r#"{
            "version": 1, "exportedAt": "2024-01-15T10:00:00Z", "appName": "TonSuuChecker",
            "includesImages": false,
            "stock": [{"id": "old-001", "timestamp": 1705312800000, "base64Images": [], "imageUrls": [],
                       "result": {"isTargetDetected": true, "truckType": "4t", "materialType": "As殻",
                                  "estimatedTonnage": 3.5, "confidenceScore": 0.85, "materialBreakdown": []}}],
            "vehicles": []
        }"#;

        let json2 = r#"{
            "version": 1, "exportedAt": "2024-01-16T10:00:00Z", "appName": "TonSuuChecker",
            "includesImages": false,
            "stock": [{"id": "new-001", "timestamp": 1705400000000, "base64Images": [], "imageUrls": [],
                       "result": {"isTargetDetected": true, "truckType": "10t", "materialType": "Co殻",
                                  "estimatedTonnage": 8.0, "confidenceScore": 0.92, "materialBreakdown": []}}],
            "vehicles": []
        }"#;

        let data1: LegacyExportData = serde_json::from_str(json1).unwrap();
        let data2: LegacyExportData = serde_json::from_str(json2).unwrap();

        // First import
        let result1 = import_legacy_data(&data1, &mut store, ImportMode::Append);
        assert_eq!(result1.history_imported, 1);
        assert_eq!(store.count(), 1);

        // Refresh import - should clear old data
        let result2 = import_legacy_data(&data2, &mut store, ImportMode::Refresh);
        assert!(result2.is_success());
        assert_eq!(result2.cleared, 1);  // 1 item was cleared
        assert_eq!(result2.history_imported, 1);
        assert_eq!(store.count(), 1);

        // Verify the new data is there (not the old)
        assert!(store.has_entry("new-001"));
        assert!(!store.has_entry("old-001"));
    }

    #[test]
    fn test_convert_to_history_entry() {
        let item = LegacyStockItem {
            id: "test-123".to_string(),
            timestamp: 1705312800000,
            base64_images: vec!["base64data...".to_string()],
            image_urls: vec![],
            actual_tonnage: Some(4.2),
            max_capacity: Some(10.0),
            memo: Some("テストメモ".to_string()),
            manifest_number: None,
            waste_type: Some("As殻".to_string()),
            destination: None,
            result: Some(LegacyEstimationResult {
                is_target_detected: true,
                truck_type: "10t".to_string(),
                license_plate: Some("品川100あ1234".to_string()),
                license_number: None,
                material_type: "As殻".to_string(),
                estimated_volume_m3: Some(5.5),
                estimated_tonnage: Some(4.0),
                estimated_max_capacity: Some(10.0),
                confidence_score: Some(0.88),
                reasoning: Some("Test reasoning".to_string()),
                ensemble_count: Some(3),
                material_breakdown: vec![],
            }),
            estimations: vec![],
            chat_history: vec![],
        };

        let entry = convert_to_history_entry(&item);

        assert_eq!(entry.image_hash, "test-123");
        assert_eq!(entry.actual_tonnage, Some(4.2));
        assert_eq!(entry.max_capacity, Some(10.0));
        assert_eq!(entry.notes, Some("テストメモ".to_string()));
        assert_eq!(entry.estimation.truck_type, "10t");
        assert_eq!(entry.estimation.material_type, "As殻");
        assert!((entry.estimation.estimated_tonnage - 4.0).abs() < 0.001);
        assert!((entry.estimation.confidence_score - 0.88).abs() < 0.001);
        assert_eq!(entry.thumbnail_base64, Some("base64data...".to_string()));
    }
}
