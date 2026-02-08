//! Repository adapters for persistence layer

use std::path::PathBuf;

use tonsuu_infra::persistence::{
    FileAnalysisHistoryRepository, FileVehicleMasterRepository, FileVehicleRepository,
    FileWeighingSlipRepository,
};
use tonsuu_store::{Store, VehicleStore};
use tonsuu_types::Result;

use crate::config::Config;

/// Open file-based analysis history repository
pub fn open_history_repo(config: &Config) -> Result<FileAnalysisHistoryRepository> {
    let store_dir = config.store_dir()?;
    FileAnalysisHistoryRepository::open(store_dir).map_err(Into::into)
}

/// Open file-based vehicle repository
pub fn open_vehicle_repo(config: &Config) -> Result<FileVehicleRepository> {
    let store_dir = config.store_dir()?;
    FileVehicleRepository::open(store_dir).map_err(Into::into)
}

/// Open Store for analysis history
pub fn open_history_store(config: &Config) -> Result<Store> {
    let store_dir = config.store_dir()?;
    Store::open(store_dir).map_err(Into::into)
}

/// Open Store for registered vehicles
pub fn open_vehicle_store(config: &Config) -> Result<VehicleStore> {
    let store_dir = config.store_dir()?;
    VehicleStore::open(store_dir).map_err(Into::into)
}

/// Open Store for analysis history at a custom directory
pub fn open_history_store_at(store_dir: PathBuf) -> Result<Store> {
    Store::open(store_dir).map_err(Into::into)
}

/// Open Store for registered vehicles at a custom directory
pub fn open_vehicle_store_at(store_dir: PathBuf) -> Result<VehicleStore> {
    VehicleStore::open(store_dir).map_err(Into::into)
}

/// Open vehicle master repository from TOML
pub fn open_vehicle_master_repo(toml_path: PathBuf) -> Result<FileVehicleMasterRepository> {
    FileVehicleMasterRepository::new(toml_path).map_err(Into::into)
}

/// Open weighing slip repository from CSV
pub fn open_weighing_slip_repo(csv_path: PathBuf) -> Result<FileWeighingSlipRepository> {
    FileWeighingSlipRepository::new(csv_path).map_err(Into::into)
}
