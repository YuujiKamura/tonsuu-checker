//! Weight calculation functions for materials
//!
//! This module provides pure weight calculation functions based on
//! volume, density, and void ratio. For material lookup functionality,
//! use `constants::weight_calculator::calculate_weight`.
//!
//! Note: Prepared for weight calculation service layer.
//! Currently unused but maintained for planned calculation feature.

#![allow(dead_code)]

use crate::domain::model::MaterialSpec;

/// Calculate weight from volume and material specification
///
/// # Formula
/// weight = volume x density x (1 - void_ratio)
///
/// # Arguments
/// * `volume_m3` - Volume in cubic meters
/// * `spec` - Material specification containing density and void ratio
///
/// # Returns
/// Weight in tonnes
///
/// # Examples
/// ```ignore
/// use crate::domain::MaterialSpec;
/// let spec = MaterialSpec { name: "土砂".to_string(), density: 1.8, void_ratio: 0.05 };
/// let weight = calculate_weight_from_spec(2.0, &spec);
/// assert!((weight - 3.42).abs() < 0.01);
/// ```
pub fn calculate_weight_from_spec(volume_m3: f64, spec: &MaterialSpec) -> f64 {
    volume_m3 * spec.density * (1.0 - spec.void_ratio)
}

/// Calculate weight with explicit density and void ratio
///
/// # Formula
/// weight = volume x density x (1 - void_ratio)
///
/// # Arguments
/// * `volume_m3` - Volume in cubic meters
/// * `density` - Material density in t/m3
/// * `void_ratio` - Void ratio (0.0 to 1.0)
///
/// # Returns
/// Weight in tonnes
///
/// # Examples
/// ```ignore
/// // 2m3 of material with density 1.8 t/m3 and 5% void ratio
/// let weight = calculate_weight_explicit(2.0, 1.8, 0.05);
/// assert!((weight - 3.42).abs() < 0.01);
/// ```
pub fn calculate_weight_explicit(volume_m3: f64, density: f64, void_ratio: f64) -> f64 {
    volume_m3 * density * (1.0 - void_ratio)
}

#[cfg(test)]
mod tests {
    use super::*;

    // Helper function to create test MaterialSpec
    fn soil_spec() -> MaterialSpec {
        MaterialSpec {
            name: "土砂".to_string(),
            density: 1.8,
            void_ratio: 0.05,
        }
    }

    fn asphalt_debris_spec() -> MaterialSpec {
        MaterialSpec {
            name: "As殻".to_string(),
            density: 2.5,
            void_ratio: 0.30,
        }
    }

    fn concrete_debris_spec() -> MaterialSpec {
        MaterialSpec {
            name: "Co殻".to_string(),
            density: 2.5,
            void_ratio: 0.30,
        }
    }

    fn open_graded_asphalt_spec() -> MaterialSpec {
        MaterialSpec {
            name: "開粒度As殻".to_string(),
            density: 2.35,
            void_ratio: 0.35,
        }
    }

    // ==========================================
    // Basic weight calculation tests using MaterialSpec
    // ==========================================

    #[test]
    fn test_weight_from_spec_soil() {
        // 2m3 of soil: 2 x 1.8 x 0.95 = 3.42t
        let weight = calculate_weight_from_spec(2.0, &soil_spec());
        assert!((weight - 3.42).abs() < 0.01);
    }

    #[test]
    fn test_weight_from_spec_asphalt_debris() {
        // 2m3 of asphalt debris: 2 x 2.5 x 0.70 = 3.5t
        let weight = calculate_weight_from_spec(2.0, &asphalt_debris_spec());
        assert!((weight - 3.5).abs() < 0.01);
    }

    #[test]
    fn test_weight_from_spec_concrete_debris() {
        // 2m3 of concrete debris: 2 x 2.5 x 0.70 = 3.5t
        let weight = calculate_weight_from_spec(2.0, &concrete_debris_spec());
        assert!((weight - 3.5).abs() < 0.01);
    }

    #[test]
    fn test_weight_from_spec_open_graded_asphalt() {
        // 2m3 of open-graded asphalt: 2 x 2.35 x 0.65 = 3.055t
        let weight = calculate_weight_from_spec(2.0, &open_graded_asphalt_spec());
        assert!((weight - 3.055).abs() < 0.01);
    }

    // ==========================================
    // Edge cases - Zero values
    // ==========================================

    #[test]
    fn test_from_spec_zero_volume() {
        // Zero volume should return zero weight
        let weight = calculate_weight_from_spec(0.0, &soil_spec());
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

    #[test]
    fn test_from_spec_zero_density() {
        // Zero density material spec
        let spec = MaterialSpec {
            name: "test".to_string(),
            density: 0.0,
            void_ratio: 0.05,
        };
        let weight = calculate_weight_from_spec(2.0, &spec);
        assert!((weight - 0.0).abs() < f64::EPSILON);
    }

    #[test]
    fn test_from_spec_void_ratio_one() {
        // 100% void ratio - all air
        let spec = MaterialSpec {
            name: "test".to_string(),
            density: 1.8,
            void_ratio: 1.0,
        };
        let weight = calculate_weight_from_spec(2.0, &spec);
        assert!((weight - 0.0).abs() < f64::EPSILON);
    }

    // ==========================================
    // Edge cases - Maximum and large values
    // ==========================================

    #[test]
    fn test_from_spec_large_volume() {
        // 100m3 of soil: 100 x 1.8 x 0.95 = 171t
        let weight = calculate_weight_from_spec(100.0, &soil_spec());
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
    fn test_from_spec_small_volume() {
        // Very small volume: 0.001m3 of soil
        // 0.001 x 1.8 x 0.95 = 0.00171t
        let weight = calculate_weight_from_spec(0.001, &soil_spec());
        assert!((weight - 0.00171).abs() < 0.0001);
    }

    // ==========================================
    // Explicit calculation tests
    // ==========================================

    #[test]
    fn test_explicit_matches_from_spec() {
        // Verify explicit calculation matches the spec-based calculation
        // Soil: density=1.8, void_ratio=0.05
        let spec_weight = calculate_weight_from_spec(2.0, &soil_spec());
        let explicit_weight = calculate_weight_explicit(2.0, 1.8, 0.05);
        assert!((spec_weight - explicit_weight).abs() < 0.01);
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

    #[test]
    fn test_from_spec_negative_volume() {
        // Negative volume (edge case - should still compute mathematically)
        let weight = calculate_weight_from_spec(-2.0, &soil_spec());
        assert!((weight - (-3.42)).abs() < 0.01);
    }
}
