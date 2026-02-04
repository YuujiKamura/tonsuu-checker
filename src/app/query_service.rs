//! Query Service - Access Stored Data
//!
//! This service provides read-only access to stored data:
//! - Analysis history
//! - Registered vehicles
//! - Accuracy statistics
//!
//! Note: This module is prepared for future GUI/API integration.
//! Currently unused but maintained for planned features.

#![allow(dead_code)]

use crate::config::Config;
use crate::store::{AccuracyStats, HistoryEntry, Store, VehicleStore};
use crate::types::{RegisteredVehicle, TruckClass};
use std::path::Path;
use thiserror::Error;

/// Errors specific to the query service
#[derive(Debug, Error)]
pub enum QueryServiceError {
    #[error("Store not accessible: {0}")]
    StoreError(String),

    #[error("Entry not found: {0}")]
    NotFound(String),
}

impl From<crate::error::Error> for QueryServiceError {
    fn from(err: crate::error::Error) -> Self {
        QueryServiceError::StoreError(err.to_string())
    }
}

// ============================================================================
// Vehicle Queries
// ============================================================================

/// Get all registered vehicles
pub fn get_vehicles(config: &Config) -> std::result::Result<Vec<RegisteredVehicle>, QueryServiceError> {
    let store = open_vehicle_store(config)?;
    Ok(store.all_vehicles().into_iter().cloned().collect())
}

/// Get vehicles filtered by company
pub fn get_vehicles_by_company(
    config: &Config,
    company: &str,
) -> std::result::Result<Vec<RegisteredVehicle>, QueryServiceError> {
    let store = open_vehicle_store(config)?;
    Ok(store
        .all_vehicles()
        .into_iter()
        .filter(|v| {
            v.company
                .as_ref()
                .map(|c| c.contains(company))
                .unwrap_or(false)
        })
        .cloned()
        .collect())
}

/// Get vehicles by truck class
pub fn get_vehicles_by_class(
    config: &Config,
    class: TruckClass,
) -> std::result::Result<Vec<RegisteredVehicle>, QueryServiceError> {
    let store = open_vehicle_store(config)?;
    Ok(store.vehicles_by_class(class).into_iter().cloned().collect())
}

/// Get a vehicle by ID
pub fn get_vehicle_by_id(
    config: &Config,
    id: &str,
) -> std::result::Result<Option<RegisteredVehicle>, QueryServiceError> {
    let store = open_vehicle_store(config)?;
    Ok(store.get_vehicle(id).cloned())
}

/// Get a vehicle by license plate (with fuzzy matching)
pub fn get_vehicle_by_plate(
    config: &Config,
    plate: &str,
) -> std::result::Result<Option<RegisteredVehicle>, QueryServiceError> {
    let store = open_vehicle_store(config)?;

    // Try exact match first
    if let Some(vehicle) = store.get_by_license_plate(plate) {
        return Ok(Some(vehicle.clone()));
    }

    // Try fuzzy match
    let normalized_plate = plate
        .replace(' ', "")
        .replace('\u{3000}', "")
        .replace('-', "");
    let plate_nums: String = normalized_plate
        .chars()
        .filter(|c| c.is_ascii_digit())
        .collect();

    for vehicle in store.all_vehicles() {
        if let Some(ref vplate) = vehicle.license_plate {
            let normalized_vplate = vplate
                .replace(' ', "")
                .replace('\u{3000}', "")
                .replace('-', "");

            if normalized_plate == normalized_vplate {
                return Ok(Some(vehicle.clone()));
            }

            let vplate_nums: String = normalized_vplate
                .chars()
                .filter(|c| c.is_ascii_digit())
                .collect();
            if plate_nums.len() >= 4 && vplate_nums.len() >= 4 {
                let plate_last4 = &plate_nums[plate_nums.len() - 4..];
                let vplate_last4 = &vplate_nums[vplate_nums.len() - 4..];
                if plate_last4 == vplate_last4 {
                    return Ok(Some(vehicle.clone()));
                }
            }
        }
    }

    Ok(None)
}

/// Get total vehicle count
pub fn get_vehicle_count(config: &Config) -> std::result::Result<usize, QueryServiceError> {
    let store = open_vehicle_store(config)?;
    Ok(store.count())
}

// ============================================================================
// History Queries
// ============================================================================

/// Get analysis history (most recent first)
pub fn get_analysis_history(
    config: &Config,
    limit: Option<usize>,
) -> std::result::Result<Vec<HistoryEntry>, QueryServiceError> {
    let store = open_history_store(config)?;
    let entries: Vec<HistoryEntry> = store.all_entries().into_iter().cloned().collect();

    Ok(match limit {
        Some(n) => entries.into_iter().take(n).collect(),
        None => entries,
    })
}

/// Get history entries with feedback (ground truth)
pub fn get_history_with_feedback(
    config: &Config,
    limit: Option<usize>,
) -> std::result::Result<Vec<HistoryEntry>, QueryServiceError> {
    let store = open_history_store(config)?;
    let entries: Vec<HistoryEntry> = store
        .entries_with_feedback()
        .into_iter()
        .cloned()
        .collect();

    Ok(match limit {
        Some(n) => entries.into_iter().take(n).collect(),
        None => entries,
    })
}

/// Get history entry by image path
pub fn get_history_by_image(
    config: &Config,
    image_path: &Path,
) -> std::result::Result<Option<HistoryEntry>, QueryServiceError> {
    let store = open_history_store(config)?;
    Ok(store.get_by_path(image_path)?.cloned())
}

/// Get history entry by hash
pub fn get_history_by_hash(
    config: &Config,
    hash: &str,
) -> std::result::Result<Option<HistoryEntry>, QueryServiceError> {
    let store = open_history_store(config)?;
    Ok(store.get_by_hash(hash).cloned())
}

/// Get total history count
pub fn get_history_count(config: &Config) -> std::result::Result<usize, QueryServiceError> {
    let store = open_history_store(config)?;
    Ok(store.count())
}

/// Get count of entries with feedback
pub fn get_feedback_count(config: &Config) -> std::result::Result<usize, QueryServiceError> {
    let store = open_history_store(config)?;
    Ok(store.feedback_count())
}

// ============================================================================
// Accuracy Queries
// ============================================================================

/// Get overall accuracy statistics
pub fn get_accuracy_stats(config: &Config) -> std::result::Result<AccuracyStats, QueryServiceError> {
    let store = open_history_store(config)?;
    Ok(store.accuracy_stats())
}

/// Get accuracy statistics grouped by truck type
pub fn get_accuracy_by_truck_type(
    config: &Config,
) -> std::result::Result<std::collections::HashMap<String, AccuracyStats>, QueryServiceError> {
    let store = open_history_store(config)?;
    Ok(store.accuracy_stats().by_truck_type())
}

/// Get accuracy statistics grouped by material type
pub fn get_accuracy_by_material_type(
    config: &Config,
) -> std::result::Result<std::collections::HashMap<String, AccuracyStats>, QueryServiceError> {
    let store = open_history_store(config)?;
    Ok(store.accuracy_stats().by_material_type())
}

// ============================================================================
// Helper Functions
// ============================================================================

fn open_vehicle_store(config: &Config) -> std::result::Result<VehicleStore, QueryServiceError> {
    let store_dir = config.store_dir().map_err(|e| {
        QueryServiceError::StoreError(format!("Failed to get store directory: {}", e))
    })?;
    VehicleStore::open(store_dir).map_err(|e| {
        QueryServiceError::StoreError(format!("Failed to open vehicle store: {}", e))
    })
}

fn open_history_store(config: &Config) -> std::result::Result<Store, QueryServiceError> {
    let store_dir = config.store_dir().map_err(|e| {
        QueryServiceError::StoreError(format!("Failed to get store directory: {}", e))
    })?;
    Store::open(store_dir).map_err(|e| {
        QueryServiceError::StoreError(format!("Failed to open history store: {}", e))
    })
}

#[cfg(test)]
mod tests {
    use super::*;

    // Note: Integration tests would require a test config and store setup
    // Unit tests for query service are limited since it primarily wraps store calls
}
