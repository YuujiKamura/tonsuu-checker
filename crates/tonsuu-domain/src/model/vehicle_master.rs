//! Vehicle master data type definitions

use serde::{Deserialize, Serialize};

/// Vehicle master data containing capacity and company information
#[allow(dead_code)]
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct VehicleMaster {
    /// 車両番号 (e.g., "1122", "1111")
    pub vehicle_number: String,
    /// 最大積載量(t)
    pub max_capacity_tons: f64,
    /// 運送会社
    pub transport_company: String,
    /// トラック種別 (4t, 10t, etc.)
    pub truck_type: Option<String>,
}
