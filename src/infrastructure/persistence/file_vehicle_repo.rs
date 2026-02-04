//! File-based vehicle repository implementation

#![allow(dead_code)]

use crate::domain::VehicleRepository;
use crate::error::{Error, Result};
use crate::types::RegisteredVehicle;
use std::cell::RefCell;
use std::collections::HashMap;
use std::fs::{self, File};
use std::io::{BufReader, BufWriter};
use std::path::PathBuf;

/// File-based implementation of VehicleRepository
///
/// Stores vehicles in a JSON file on disk.
pub struct FileVehicleRepository {
    store_path: PathBuf,
    vehicles: RefCell<HashMap<String, RegisteredVehicle>>,
}

impl FileVehicleRepository {
    /// Create or load a vehicle repository
    pub fn open(store_dir: PathBuf) -> Result<Self> {
        fs::create_dir_all(&store_dir)?;
        let store_path = store_dir.join("vehicles.json");

        let vehicles = if store_path.exists() {
            let file = File::open(&store_path)?;
            let reader = BufReader::new(file);
            serde_json::from_reader(reader).unwrap_or_default()
        } else {
            HashMap::new()
        };

        Ok(Self {
            store_path,
            vehicles: RefCell::new(vehicles),
        })
    }

    /// Save store to disk
    fn persist(&self) -> Result<()> {
        let file = File::create(&self.store_path)?;
        let writer = BufWriter::new(file);
        serde_json::to_writer_pretty(writer, &*self.vehicles.borrow())?;
        Ok(())
    }

    /// Add a new vehicle and return its ID
    pub fn add_vehicle(&self, vehicle: RegisteredVehicle) -> Result<String> {
        let id = vehicle.id.clone();
        self.vehicles.borrow_mut().insert(id.clone(), vehicle);
        self.persist()?;
        Ok(id)
    }

    /// Remove a vehicle by ID
    pub fn remove_vehicle(&self, id: &str) -> Result<bool> {
        let removed = self.vehicles.borrow_mut().remove(id).is_some();
        if removed {
            self.persist()?;
        }
        Ok(removed)
    }

    /// Get a vehicle by ID
    pub fn get_vehicle(&self, id: &str) -> Option<RegisteredVehicle> {
        self.vehicles.borrow().get(id).cloned()
    }

    /// Get vehicles by truck class
    pub fn vehicles_by_class(&self, class: crate::types::TruckClass) -> Vec<RegisteredVehicle> {
        self.vehicles
            .borrow()
            .values()
            .filter(|v| v.truck_class() == class)
            .cloned()
            .collect()
    }

    /// Get total vehicle count
    pub fn count(&self) -> usize {
        self.vehicles.borrow().len()
    }

    /// Update a vehicle
    pub fn update_vehicle(&self, vehicle: RegisteredVehicle) -> Result<bool> {
        let mut vehicles = self.vehicles.borrow_mut();
        if vehicles.contains_key(&vehicle.id) {
            vehicles.insert(vehicle.id.clone(), vehicle);
            drop(vehicles);
            self.persist()?;
            Ok(true)
        } else {
            Ok(false)
        }
    }
}

impl VehicleRepository for FileVehicleRepository {
    fn save(&self, vehicle: &RegisteredVehicle) -> std::result::Result<(), Error> {
        let mut vehicles = self.vehicles.borrow_mut();
        vehicles.insert(vehicle.id.clone(), vehicle.clone());
        drop(vehicles);
        self.persist()
    }

    fn find_by_plate(&self, plate: &str) -> std::result::Result<Option<RegisteredVehicle>, Error> {
        let result = self.vehicles.borrow().values().find(|v| {
            v.license_plate
                .as_ref()
                .map(|p| p == plate)
                .unwrap_or(false)
        }).cloned();
        Ok(result)
    }

    fn find_all(&self) -> std::result::Result<Vec<RegisteredVehicle>, Error> {
        let mut vehicles: Vec<_> = self.vehicles.borrow().values().cloned().collect();
        vehicles.sort_by(|a, b| a.name.cmp(&b.name));
        Ok(vehicles)
    }
}
