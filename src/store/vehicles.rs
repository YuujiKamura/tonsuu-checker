//! Vehicle store for registered vehicles

use crate::error::Result;
use crate::types::{RegisteredVehicle, TruckClass};
use std::collections::HashMap;
use std::fs::{self, File};
use std::io::{BufReader, BufWriter};
use std::path::PathBuf;

/// Persistent store for registered vehicles
pub struct VehicleStore {
    store_path: PathBuf,
    vehicles: HashMap<String, RegisteredVehicle>,
}

impl VehicleStore {
    /// Create or load a vehicle store
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

        Ok(Self { store_path, vehicles })
    }

    /// Save store to disk
    fn save(&self) -> Result<()> {
        let file = File::create(&self.store_path)?;
        let writer = BufWriter::new(file);
        serde_json::to_writer_pretty(writer, &self.vehicles)?;
        Ok(())
    }

    /// Add a new vehicle
    pub fn add_vehicle(&mut self, vehicle: RegisteredVehicle) -> Result<String> {
        let id = vehicle.id.clone();
        self.vehicles.insert(id.clone(), vehicle);
        self.save()?;
        Ok(id)
    }

    /// Remove a vehicle by ID
    #[allow(dead_code)]
    pub fn remove_vehicle(&mut self, id: &str) -> Result<bool> {
        let removed = self.vehicles.remove(id).is_some();
        if removed {
            self.save()?;
        }
        Ok(removed)
    }

    /// Get a vehicle by ID
    #[allow(dead_code)]
    pub fn get_vehicle(&self, id: &str) -> Option<&RegisteredVehicle> {
        self.vehicles.get(id)
    }

    /// Find vehicle by license plate
    pub fn get_by_license_plate(&self, plate: &str) -> Option<&RegisteredVehicle> {
        self.vehicles.values().find(|v| {
            v.license_plate
                .as_ref()
                .map(|p| p == plate)
                .unwrap_or(false)
        })
    }

    /// Get all vehicles sorted by name
    pub fn all_vehicles(&self) -> Vec<&RegisteredVehicle> {
        let mut vehicles: Vec<_> = self.vehicles.values().collect();
        vehicles.sort_by(|a, b| a.name.cmp(&b.name));
        vehicles
    }

    /// Get vehicles by truck class
    #[allow(dead_code)]
    pub fn vehicles_by_class(&self, class: TruckClass) -> Vec<&RegisteredVehicle> {
        self.vehicles
            .values()
            .filter(|v| v.truck_class() == class)
            .collect()
    }

    /// Get total vehicle count
    pub fn count(&self) -> usize {
        self.vehicles.len()
    }

    /// Update a vehicle
    #[allow(dead_code)]
    pub fn update_vehicle(&mut self, vehicle: RegisteredVehicle) -> Result<bool> {
        if self.vehicles.contains_key(&vehicle.id) {
            self.vehicles.insert(vehicle.id.clone(), vehicle);
            self.save()?;
            Ok(true)
        } else {
            Ok(false)
        }
    }
}
