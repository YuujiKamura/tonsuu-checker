//! Weight calculation functions for materials

#![allow(dead_code)]

use super::materials::get_material_spec;

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
