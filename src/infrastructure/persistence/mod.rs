//! Persistence implementations
//!
//! This module provides file-based implementations of the repository traits.

mod file_analysis_history_repo;
mod file_vehicle_master_repo;
mod file_vehicle_repo;
mod file_weighing_slip_repo;

#[allow(unused_imports)]
pub use file_analysis_history_repo::FileAnalysisHistoryRepository;
#[allow(unused_imports)]
pub use file_vehicle_master_repo::FileVehicleMasterRepository;
#[allow(unused_imports)]
pub use file_vehicle_repo::FileVehicleRepository;
#[allow(unused_imports)]
pub use file_weighing_slip_repo::FileWeighingSlipRepository;
