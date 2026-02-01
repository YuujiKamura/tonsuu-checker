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

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_material_lookup() {
        assert!(get_material_spec("土砂").is_some());
        assert!(get_material_spec("As殻").is_some());
        assert!(get_material_spec("Co殻").is_some());
    }
}
