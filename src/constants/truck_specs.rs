//! Truck specifications for Japanese dump trucks

#![allow(dead_code)]

use crate::types::TruckSpec;
use std::collections::HashMap;
use std::sync::LazyLock;

/// Standard truck specifications
pub static TRUCK_SPECS: LazyLock<HashMap<&'static str, TruckSpec>> = LazyLock::new(|| {
    let mut m = HashMap::new();

    m.insert(
        "2t",
        TruckSpec {
            name: "2tダンプ".to_string(),
            max_capacity: 2.0,
            bed_length: 3.0,
            bed_width: 1.6,
            bed_height: 0.32,
            level_volume: 1.5,
            heap_volume: 2.0,
        },
    );

    m.insert(
        "4t",
        TruckSpec {
            name: "4tダンプ".to_string(),
            max_capacity: 4.0,
            bed_length: 3.4,
            bed_width: 2.06,
            bed_height: 0.34,
            level_volume: 2.0,
            heap_volume: 2.4,
        },
    );

    m.insert(
        "増トン",
        TruckSpec {
            name: "増トンダンプ".to_string(),
            max_capacity: 6.5,
            bed_length: 4.0,
            bed_width: 2.2,
            bed_height: 0.40,
            level_volume: 3.5,
            heap_volume: 4.5,
        },
    );

    m.insert(
        "10t",
        TruckSpec {
            name: "10tダンプ".to_string(),
            max_capacity: 10.0,
            bed_length: 5.3,
            bed_width: 2.3,
            bed_height: 0.50,
            level_volume: 6.0,
            heap_volume: 7.8,
        },
    );

    m
});

/// Get truck spec by type name
pub fn get_truck_spec(truck_type: &str) -> Option<&'static TruckSpec> {
    // Normalize truck type name
    let normalized = truck_type
        .replace("ダンプ", "")
        .replace("t", "")
        .trim()
        .to_string();

    // Try direct lookup first
    if let Some(spec) = TRUCK_SPECS.get(truck_type) {
        return Some(spec);
    }

    // Try normalized lookup
    for (key, spec) in TRUCK_SPECS.iter() {
        if key.replace("t", "") == normalized {
            return Some(spec);
        }
        if spec.name.contains(truck_type) {
            return Some(spec);
        }
    }

    None
}

/// Get max capacity for a truck type
pub fn get_max_capacity(truck_type: &str) -> Option<f64> {
    get_truck_spec(truck_type).map(|s| s.max_capacity)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_get_truck_spec() {
        assert!(get_truck_spec("4t").is_some());
        assert!(get_truck_spec("10t").is_some());
        assert!(get_truck_spec("増トン").is_some());
    }

    #[test]
    fn test_4t_spec() {
        let spec = get_truck_spec("4t").unwrap();
        assert_eq!(spec.max_capacity, 4.0);
        assert_eq!(spec.level_volume, 2.0);
    }
}
