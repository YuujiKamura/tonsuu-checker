//! Material specifications for weight calculation

#![allow(dead_code)]

use crate::types::MaterialSpec;
use std::collections::HashMap;
use std::sync::LazyLock;

/// Material specifications
pub static MATERIALS: LazyLock<HashMap<&'static str, MaterialSpec>> = LazyLock::new(|| {
    let mut m = HashMap::new();

    m.insert(
        "土砂",
        MaterialSpec {
            name: "土砂".to_string(),
            density: 1.8,
            void_ratio: 0.05, // 3-8%
        },
    );

    m.insert(
        "As殻",
        MaterialSpec {
            name: "As殻".to_string(),
            density: 2.5,
            void_ratio: 0.30, // 25-35%
        },
    );

    m.insert(
        "Co殻",
        MaterialSpec {
            name: "Co殻".to_string(),
            density: 2.5,
            void_ratio: 0.30, // 25-35%
        },
    );

    m.insert(
        "開粒度As殻",
        MaterialSpec {
            name: "開粒度As殻".to_string(),
            density: 2.35,
            void_ratio: 0.35, // 30-40%
        },
    );

    m
});

/// Get material spec by name
pub fn get_material_spec(material_type: &str) -> Option<&'static MaterialSpec> {
    MATERIALS.get(material_type)
}

/// Calculate weight from volume and material
///
/// Formula: weight = volume × density × (1 - void_ratio)
pub fn calculate_weight(volume_m3: f64, material_type: &str) -> Option<f64> {
    get_material_spec(material_type).map(|spec| {
        volume_m3 * spec.density * (1.0 - spec.void_ratio)
    })
}

/// Calculate weight with explicit density and void ratio
pub fn calculate_weight_explicit(volume_m3: f64, density: f64, void_ratio: f64) -> f64 {
    volume_m3 * density * (1.0 - void_ratio)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_material_lookup() {
        assert!(get_material_spec("土砂").is_some());
        assert!(get_material_spec("As殻").is_some());
        assert!(get_material_spec("Co殻").is_some());
    }

    #[test]
    fn test_weight_calculation() {
        // 2m³ of soil: 2 × 1.8 × 0.95 = 3.42t
        let weight = calculate_weight(2.0, "土砂").unwrap();
        assert!((weight - 3.42).abs() < 0.01);
    }

    #[test]
    fn test_asphalt_debris() {
        // 2m³ of asphalt debris: 2 × 2.5 × 0.70 = 3.5t
        let weight = calculate_weight(2.0, "As殻").unwrap();
        assert!((weight - 3.5).abs() < 0.01);
    }
}
