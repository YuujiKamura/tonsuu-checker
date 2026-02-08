//! File-based implementation of VehicleMasterRepository
//!
//! Note: Prepared for vehicle master data repository.
//! Currently unused but maintained for planned master data feature.

#![allow(dead_code)]

use std::path::PathBuf;

use tonsuu_domain::model::VehicleMaster;
use tonsuu_domain::repository::VehicleMasterRepository;
use tonsuu_types::Error;

use crate::vehicle_master_loader::VehicleMasterLoader;

/// File-based VehicleMaster repository (TOML)
pub struct FileVehicleMasterRepository {
    toml_path: PathBuf,
    loader: VehicleMasterLoader,
}

impl FileVehicleMasterRepository {
    /// Create a new repository from a TOML file path
    pub fn new(toml_path: PathBuf) -> Result<Self, Error> {
        let loader = VehicleMasterLoader::load_from_file(&toml_path)?;
        Ok(Self { toml_path, loader })
    }

    /// Get the TOML path
    pub fn toml_path(&self) -> &PathBuf {
        &self.toml_path
    }

    /// Reload data from TOML
    pub fn reload(&mut self) -> Result<(), Error> {
        self.loader = VehicleMasterLoader::load_from_file(&self.toml_path)?;
        Ok(())
    }
}

impl VehicleMasterRepository for FileVehicleMasterRepository {
    fn find_all(&self) -> Result<Vec<VehicleMaster>, Error> {
        Ok(self.loader.all_vehicles().into_iter().cloned().collect())
    }

    fn find_by_number(&self, vehicle_number: &str) -> Result<Option<VehicleMaster>, Error> {
        Ok(self.loader.get_vehicle(vehicle_number).cloned())
    }
}
