//! Material-related type definitions

use serde::{Deserialize, Serialize};

/// Material properties
/// Note: Prepared for material-based weight calculation. Currently unused.
#[allow(dead_code)]
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct MaterialSpec {
    /// Display name
    pub name: String,
    /// Density in t/m³
    pub density: f64,
    /// Void ratio (0.0 - 1.0)
    pub void_ratio: f64,
}
