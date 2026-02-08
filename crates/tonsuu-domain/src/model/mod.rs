//! Domain model types

pub mod material;
pub mod truck;
pub mod vehicle_master;
pub mod weighing_slip;

pub use material::MaterialSpec;
pub use truck::TruckSpec;
pub use vehicle_master::VehicleMaster;
pub use weighing_slip::WeighingSlip;
