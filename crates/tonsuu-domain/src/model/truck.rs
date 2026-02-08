//! Truck-related type definitions

use serde::{Deserialize, Serialize};

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
