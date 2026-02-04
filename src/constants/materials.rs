//! Material specifications for weight calculation
//!
//! Note: Prepared for material-based weight calculation.
//! Currently unused but maintained for planned weight calculation feature.

#![allow(dead_code)]

use crate::config::load_material_specs;
use crate::domain::MaterialSpec;

/// Get material spec by name
pub fn get_material_spec(material_type: &str) -> Option<&'static MaterialSpec> {
    let loaded = load_material_specs().ok()?;
    loaded.specs.get(material_type)
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
