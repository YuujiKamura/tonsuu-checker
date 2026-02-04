//! Vehicle master data loader from TOML configuration
//!
//! Note: Prepared for loading vehicle master data from TOML files.
//! Currently unused but maintained for planned master data import feature.

#![allow(dead_code)]

use crate::domain::model::VehicleMaster;
use crate::error::{ConfigError, Error, Result};
use serde::Deserialize;
use std::collections::HashMap;
use std::fs;
use std::path::Path;

/// Container for parsing vehicles.toml
#[derive(Debug, Deserialize)]
struct VehicleMasterConfig {
    vehicles: Vec<VehicleMaster>,
}

/// Vehicle master data repository loaded from TOML
#[derive(Debug)]
pub struct VehicleMasterLoader {
    /// Map of vehicle_number to VehicleMaster
    vehicles: HashMap<String, VehicleMaster>,
}

impl VehicleMasterLoader {
    /// Load vehicle master data from a TOML file
    pub fn load_from_file(path: &Path) -> Result<Self> {
        let content = fs::read_to_string(path).map_err(|e| {
            Error::Config(ConfigError::ParseError(format!(
                "Failed to read vehicle master file: {}",
                e
            )))
        })?;

        Self::load_from_str(&content)
    }

    /// Load vehicle master data from TOML string
    pub fn load_from_str(toml_content: &str) -> Result<Self> {
        let config: VehicleMasterConfig = toml::from_str(toml_content).map_err(|e| {
            Error::Config(ConfigError::ParseError(format!(
                "Failed to parse vehicle master TOML: {}",
                e
            )))
        })?;

        let vehicles = config
            .vehicles
            .into_iter()
            .map(|v| (v.vehicle_number.clone(), v))
            .collect();

        Ok(Self { vehicles })
    }

    /// Look up max capacity by vehicle number
    ///
    /// Returns None if the vehicle is not found in the master data
    pub fn get_max_capacity(&self, vehicle_number: &str) -> Option<f64> {
        self.vehicles
            .get(vehicle_number)
            .map(|v| v.max_capacity_tons)
    }

    /// Look up vehicle master by vehicle number
    pub fn get_vehicle(&self, vehicle_number: &str) -> Option<&VehicleMaster> {
        self.vehicles.get(vehicle_number)
    }

    /// Get all vehicles
    pub fn all_vehicles(&self) -> Vec<&VehicleMaster> {
        self.vehicles.values().collect()
    }

    /// Get transport company by vehicle number
    pub fn get_transport_company(&self, vehicle_number: &str) -> Option<&str> {
        self.vehicles
            .get(vehicle_number)
            .map(|v| v.transport_company.as_str())
    }

    /// Check if a vehicle number exists in the master data
    pub fn has_vehicle(&self, vehicle_number: &str) -> bool {
        self.vehicles.contains_key(vehicle_number)
    }

    /// Get the total number of registered vehicles
    pub fn count(&self) -> usize {
        self.vehicles.len()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    const TEST_TOML: &str = r#"
[[vehicles]]
vehicle_number = "1122"
max_capacity_tons = 3.75
transport_company = "松尾運搬社"

[[vehicles]]
vehicle_number = "1111"
max_capacity_tons = 3.5
transport_company = "松尾運搬社"
truck_type = "4t"
"#;

    #[test]
    fn test_load_from_str() {
        let loader = VehicleMasterLoader::load_from_str(TEST_TOML).unwrap();
        assert_eq!(loader.count(), 2);
    }

    #[test]
    fn test_get_max_capacity() {
        let loader = VehicleMasterLoader::load_from_str(TEST_TOML).unwrap();
        assert_eq!(loader.get_max_capacity("1122"), Some(3.75));
        assert_eq!(loader.get_max_capacity("1111"), Some(3.5));
        assert_eq!(loader.get_max_capacity("9999"), None);
    }

    #[test]
    fn test_get_transport_company() {
        let loader = VehicleMasterLoader::load_from_str(TEST_TOML).unwrap();
        assert_eq!(loader.get_transport_company("1122"), Some("松尾運搬社"));
        assert_eq!(loader.get_transport_company("9999"), None);
    }

    #[test]
    fn test_get_vehicle() {
        let loader = VehicleMasterLoader::load_from_str(TEST_TOML).unwrap();
        let vehicle = loader.get_vehicle("1111").unwrap();
        assert_eq!(vehicle.truck_type, Some("4t".to_string()));
    }
}
