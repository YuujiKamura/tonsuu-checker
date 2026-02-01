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

/// Truck type aliases for flexible input matching
pub static TRUCK_ALIASES: LazyLock<HashMap<&'static str, &'static str>> = LazyLock::new(|| {
    let mut m = HashMap::new();

    // 2-ton aliases
    m.insert("2トン", "2t");
    m.insert("2トンダンプ", "2t");
    m.insert("2t ダンプ", "2t");
    m.insert("2tダンプ", "2t");

    // 4-ton aliases
    m.insert("4トン", "4t");
    m.insert("4トンダンプ", "4t");
    m.insert("4t ダンプ", "4t");
    m.insert("4tダンプ", "4t");

    // 10-ton aliases
    m.insert("10トン", "10t");
    m.insert("10トンダンプ", "10t");
    m.insert("10t ダンプ", "10t");
    m.insert("10tダンプ", "10t");

    // Increased-ton (増トン) aliases
    m.insert("増トンダンプ", "増トン");
    m.insert("増t", "増トン");
    m.insert("増", "増トン");

    m
});

/// Get truck spec by type name
pub fn get_truck_spec(truck_type: &str) -> Option<&'static TruckSpec> {
    let trimmed = truck_type.trim();

    // Step 1: Try direct lookup in TRUCK_SPECS first
    if let Some(spec) = TRUCK_SPECS.get(trimmed) {
        return Some(spec);
    }

    // Step 2: Try alias resolution
    if let Some(&canonical_name) = TRUCK_ALIASES.get(trimmed) {
        if let Some(spec) = TRUCK_SPECS.get(canonical_name) {
            return Some(spec);
        }
    }

    // Step 3: Case-insensitive alias lookup (for mixed case inputs)
    let lower_input = trimmed.to_lowercase();
    for (alias, canonical) in TRUCK_ALIASES.iter() {
        if alias.to_lowercase() == lower_input {
            if let Some(spec) = TRUCK_SPECS.get(canonical) {
                return Some(spec);
            }
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

    #[test]
    fn test_alias_2ton() {
        // Test various aliases for 2-ton truck
        assert_eq!(
            get_truck_spec("2トン").map(|s| s.max_capacity),
            Some(2.0)
        );
        assert_eq!(
            get_truck_spec("2トンダンプ").map(|s| s.max_capacity),
            Some(2.0)
        );
        assert_eq!(
            get_truck_spec("2tダンプ").map(|s| s.max_capacity),
            Some(2.0)
        );
    }

    #[test]
    fn test_alias_4ton() {
        // Test various aliases for 4-ton truck
        assert_eq!(
            get_truck_spec("4トン").map(|s| s.max_capacity),
            Some(4.0)
        );
        assert_eq!(
            get_truck_spec("4トンダンプ").map(|s| s.max_capacity),
            Some(4.0)
        );
        assert_eq!(
            get_truck_spec("4tダンプ").map(|s| s.max_capacity),
            Some(4.0)
        );
    }

    #[test]
    fn test_alias_10ton() {
        // Test various aliases for 10-ton truck
        assert_eq!(
            get_truck_spec("10トン").map(|s| s.max_capacity),
            Some(10.0)
        );
        assert_eq!(
            get_truck_spec("10トンダンプ").map(|s| s.max_capacity),
            Some(10.0)
        );
        assert_eq!(
            get_truck_spec("10tダンプ").map(|s| s.max_capacity),
            Some(10.0)
        );
    }

    #[test]
    fn test_alias_masutton() {
        // Test various aliases for 増トン (increased-ton) truck
        assert_eq!(
            get_truck_spec("増").map(|s| s.max_capacity),
            Some(6.5)
        );
        assert_eq!(
            get_truck_spec("増t").map(|s| s.max_capacity),
            Some(6.5)
        );
        assert_eq!(
            get_truck_spec("増トンダンプ").map(|s| s.max_capacity),
            Some(6.5)
        );
    }

    #[test]
    fn test_case_insensitive_lookup() {
        // Test case-insensitive alias matching
        assert_eq!(
            get_truck_spec("2t").map(|s| s.max_capacity),
            Some(2.0)
        );
    }

    #[test]
    fn test_whitespace_trimming() {
        // Test that leading/trailing whitespace is handled
        assert_eq!(
            get_truck_spec("  2t  ").map(|s| s.max_capacity),
            Some(2.0)
        );
        assert_eq!(
            get_truck_spec("  4トン  ").map(|s| s.max_capacity),
            Some(4.0)
        );
    }

    #[test]
    fn test_invalid_truck_type() {
        // Test that invalid truck types return None
        assert!(get_truck_spec("99t").is_none());
        assert!(get_truck_spec("invalid").is_none());
    }
}
