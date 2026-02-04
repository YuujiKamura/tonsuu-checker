//! Truck specifications for Japanese dump trucks

use crate::config::load_truck_specs;
use crate::domain::TruckSpec;

/// Get truck spec by type name
pub fn get_truck_spec(truck_type: &str) -> Option<&'static TruckSpec> {
    let loaded = load_truck_specs().ok()?;
    let trimmed = truck_type.trim();

    // Step 1: Try direct lookup in specs first
    if let Some(spec) = loaded.specs.get(trimmed) {
        return Some(spec);
    }

    // Step 2: Try alias resolution
    if let Some(canonical_name) = loaded.aliases.get(trimmed) {
        if let Some(spec) = loaded.specs.get(canonical_name) {
            return Some(spec);
        }
    }

    // Step 3: Case-insensitive alias lookup (for mixed case inputs)
    let lower_input = trimmed.to_lowercase();
    for (alias, canonical) in loaded.aliases.iter() {
        if alias.to_lowercase() == lower_input {
            if let Some(spec) = loaded.specs.get(canonical) {
                return Some(spec);
            }
        }
    }

    None
}

/// Get max capacity for a truck type
#[allow(dead_code)]
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
