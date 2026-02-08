//! Repository trait definitions for data persistence

use chrono::NaiveDate;

use crate::model::{VehicleMaster, WeighingSlip};
use tonsuu_types::Error;
use tonsuu_types::{HistoryEntry, RegisteredVehicle};

/// Repository for analysis history entries
#[allow(dead_code)]
pub trait AnalysisHistoryRepository {
    /// Save an analysis result
    fn save(&self, result: &HistoryEntry) -> Result<(), Error>;

    /// Find an analysis entry by its hash ID
    fn find_by_id(&self, id: &str) -> Result<Option<HistoryEntry>, Error>;

    /// Find all analysis entries
    fn find_all(&self) -> Result<Vec<HistoryEntry>, Error>;
}

/// Repository for registered vehicles
#[allow(dead_code)]
pub trait VehicleRepository {
    /// Save a vehicle
    fn save(&self, vehicle: &RegisteredVehicle) -> Result<(), Error>;

    /// Find a vehicle by license plate
    fn find_by_plate(&self, plate: &str) -> Result<Option<RegisteredVehicle>, Error>;

    /// Find all vehicles
    fn find_all(&self) -> Result<Vec<RegisteredVehicle>, Error>;
}

/// Repository for weighing slips (計量伝票)
#[allow(dead_code)]
pub trait WeighingSlipRepository {
    /// Load all weighing slips
    fn find_all(&self) -> Result<Vec<WeighingSlip>, Error>;

    /// Find weighing slips by date
    fn find_by_date(&self, date: NaiveDate) -> Result<Vec<WeighingSlip>, Error>;

    /// Find weighing slips by site name
    fn find_by_site(&self, site_name: &str) -> Result<Vec<WeighingSlip>, Error>;

    /// Find weighing slips by vehicle number
    fn find_by_vehicle(&self, vehicle_number: &str) -> Result<Vec<WeighingSlip>, Error>;

    /// Find overloaded slips only
    fn find_overloaded(&self) -> Result<Vec<WeighingSlip>, Error>;
}

/// Repository for vehicle master data (車両マスタ)
#[allow(dead_code)]
pub trait VehicleMasterRepository {
    /// Load all vehicle master entries
    fn find_all(&self) -> Result<Vec<VehicleMaster>, Error>;

    /// Find by vehicle number
    fn find_by_number(&self, vehicle_number: &str) -> Result<Option<VehicleMaster>, Error>;
}
