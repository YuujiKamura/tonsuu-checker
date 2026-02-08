//! Weight calculation functions for materials

#![allow(dead_code)]

use crate::model::MaterialSpec;

pub fn calculate_weight_from_spec(volume_m3: f64, spec: &MaterialSpec) -> f64 {
    volume_m3 * spec.density * (1.0 - spec.void_ratio)
}

pub fn calculate_weight_explicit(volume_m3: f64, density: f64, void_ratio: f64) -> f64 {
    volume_m3 * density * (1.0 - void_ratio)
}

#[cfg(test)]
mod tests {
    use super::*;

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

    #[test]
    fn test_weight_from_spec_soil() {
        let weight = calculate_weight_from_spec(2.0, &soil_spec());
        assert!((weight - 3.42).abs() < 0.01);
    }

    #[test]
    fn test_weight_from_spec_asphalt_debris() {
        let weight = calculate_weight_from_spec(2.0, &asphalt_debris_spec());
        assert!((weight - 3.5).abs() < 0.01);
    }

    #[test]
    fn test_weight_from_spec_concrete_debris() {
        let weight = calculate_weight_from_spec(2.0, &concrete_debris_spec());
        assert!((weight - 3.5).abs() < 0.01);
    }

    #[test]
    fn test_weight_from_spec_open_graded_asphalt() {
        let weight = calculate_weight_from_spec(2.0, &open_graded_asphalt_spec());
        assert!((weight - 3.055).abs() < 0.01);
    }

    #[test]
    fn test_from_spec_zero_volume() {
        let weight = calculate_weight_from_spec(0.0, &soil_spec());
        assert!((weight - 0.0).abs() < f64::EPSILON);
    }

    #[test]
    fn test_explicit_matches_from_spec() {
        let spec_weight = calculate_weight_from_spec(2.0, &soil_spec());
        let explicit_weight = calculate_weight_explicit(2.0, 1.8, 0.05);
        assert!((spec_weight - explicit_weight).abs() < 0.01);
    }
}
