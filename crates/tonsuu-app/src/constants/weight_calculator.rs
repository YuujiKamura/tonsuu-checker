//! Weight calculation functions for materials
//!
//! This module provides weight calculation with material lookup.
//! The core calculation logic is in `domain::service::weight_calculator`.
//!
//! For direct calculation without lookup, use:
//! - `domain::service::weight_calculator::calculate_weight_explicit`
//! - `domain::service::weight_calculator::calculate_weight_from_spec`
//!
//! Note: Prepared for material-based weight calculation.
//! Currently unused but maintained for planned weight calculation feature.

#![allow(dead_code)]

use super::materials::get_material_spec;
use tonsuu_domain::service::weight_calculator as service;

// Re-export from domain service for convenience
pub use service::{calculate_weight_explicit, calculate_weight_from_spec};

/// Calculate weight from volume and material name
///
/// Looks up the material specification and delegates to the service layer.
///
/// # Formula
/// weight = volume x density x (1 - void_ratio)
///
/// # Arguments
/// * `volume_m3` - Volume in cubic meters
/// * `material_type` - Material type name (e.g., "土砂", "As殻", "Co殻")
///
/// # Returns
/// * `Some(weight)` - Weight in tonnes if material type is found
/// * `None` - If material type is not found in the specification
pub fn calculate_weight(volume_m3: f64, material_type: &str) -> Option<f64> {
    get_material_spec(material_type).map(|spec| {
        service::calculate_weight_from_spec(volume_m3, spec)
    })
}

#[cfg(test)]
mod tests {
    use super::*;

    // ==========================================
    // Basic weight calculation tests (with material lookup)
    // ==========================================

    #[test]
    fn test_weight_calculation() {
        // 2m3 of soil: 2 x 1.8 x 0.95 = 3.42t
        let weight = calculate_weight(2.0, "土砂").unwrap();
        assert!((weight - 3.42).abs() < 0.01);
    }

    #[test]
    fn test_asphalt_debris() {
        // 2m3 of asphalt debris: 2 x 2.5 x 0.70 = 3.5t
        let weight = calculate_weight(2.0, "As殻").unwrap();
        assert!((weight - 3.5).abs() < 0.01);
    }

    #[test]
    fn test_concrete_debris() {
        // 2m3 of concrete debris: 2 x 2.5 x 0.70 = 3.5t
        let weight = calculate_weight(2.0, "Co殻").unwrap();
        assert!((weight - 3.5).abs() < 0.01);
    }

    #[test]
    fn test_open_graded_asphalt() {
        // 2m3 of open-graded asphalt: 2 x 2.35 x 0.65 = 3.055t
        let weight = calculate_weight(2.0, "開粒度As殻").unwrap();
        assert!((weight - 3.055).abs() < 0.01);
    }

    // ==========================================
    // Edge cases - Zero values
    // ==========================================

    #[test]
    fn test_zero_volume() {
        // Zero volume should return zero weight
        let weight = calculate_weight(0.0, "土砂").unwrap();
        assert!((weight - 0.0).abs() < f64::EPSILON);
    }

    #[test]
    fn test_explicit_zero_volume() {
        let weight = calculate_weight_explicit(0.0, 1.8, 0.05);
        assert!((weight - 0.0).abs() < f64::EPSILON);
    }

    #[test]
    fn test_explicit_zero_density() {
        // Zero density should return zero weight
        let weight = calculate_weight_explicit(2.0, 0.0, 0.05);
        assert!((weight - 0.0).abs() < f64::EPSILON);
    }

    #[test]
    fn test_explicit_void_ratio_one() {
        // Void ratio of 1.0 means all void, weight should be zero
        let weight = calculate_weight_explicit(2.0, 1.8, 1.0);
        assert!((weight - 0.0).abs() < f64::EPSILON);
    }

    #[test]
    fn test_explicit_void_ratio_zero() {
        // Void ratio of 0.0 means no void, full density
        // 2 x 1.8 x 1.0 = 3.6t
        let weight = calculate_weight_explicit(2.0, 1.8, 0.0);
        assert!((weight - 3.6).abs() < 0.01);
    }

    // ==========================================
    // Edge cases - Maximum and large values
    // ==========================================

    #[test]
    fn test_large_volume() {
        // 100m3 of soil: 100 x 1.8 x 0.95 = 171t
        let weight = calculate_weight(100.0, "土砂").unwrap();
        assert!((weight - 171.0).abs() < 0.1);
    }

    #[test]
    fn test_explicit_large_values() {
        // Large volume with high density
        // 1000 x 10.0 x 0.9 = 9000t
        let weight = calculate_weight_explicit(1000.0, 10.0, 0.1);
        assert!((weight - 9000.0).abs() < 0.1);
    }

    #[test]
    fn test_small_volume() {
        // Very small volume: 0.001m3 of soil
        // 0.001 x 1.8 x 0.95 = 0.00171t
        let weight = calculate_weight(0.001, "土砂").unwrap();
        assert!((weight - 0.00171).abs() < 0.0001);
    }

    // ==========================================
    // Invalid material types
    // ==========================================

    #[test]
    fn test_unknown_material() {
        let result = calculate_weight(2.0, "unknown_material");
        assert!(result.is_none());
    }

    #[test]
    fn test_empty_material_string() {
        let result = calculate_weight(2.0, "");
        assert!(result.is_none());
    }

    // ==========================================
    // Explicit calculation tests
    // ==========================================

    #[test]
    fn test_explicit_calculation_matches_lookup() {
        // Verify explicit calculation matches the lookup-based calculation
        // Soil: density=1.8, void_ratio=0.05
        let lookup_weight = calculate_weight(2.0, "土砂").unwrap();
        let explicit_weight = calculate_weight_explicit(2.0, 1.8, 0.05);
        assert!((lookup_weight - explicit_weight).abs() < 0.01);
    }

    #[test]
    fn test_explicit_calculation_formula() {
        // Verify the formula: weight = volume x density x (1 - void_ratio)
        let volume = 5.0;
        let density = 2.3;
        let void_ratio = 0.25;
        let expected = volume * density * (1.0 - void_ratio);
        let actual = calculate_weight_explicit(volume, density, void_ratio);
        assert!((actual - expected).abs() < f64::EPSILON);
    }

    // ==========================================
    // Negative value edge cases
    // ==========================================

    #[test]
    fn test_negative_volume_explicit() {
        // Negative volume (edge case - should still compute mathematically)
        let weight = calculate_weight_explicit(-2.0, 1.8, 0.05);
        assert!((weight - (-3.42)).abs() < 0.01);
    }

    #[test]
    fn test_negative_void_ratio_explicit() {
        // Negative void ratio (edge case - should compute but unrealistic)
        // 2 x 1.8 x (1 - (-0.1)) = 2 x 1.8 x 1.1 = 3.96
        let weight = calculate_weight_explicit(2.0, 1.8, -0.1);
        assert!((weight - 3.96).abs() < 0.01);
    }
}
