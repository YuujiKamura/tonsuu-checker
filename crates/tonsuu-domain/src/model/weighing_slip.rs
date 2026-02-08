use serde::{Deserialize, Serialize};

#[allow(dead_code)]
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct WeighingSlip {
    pub slip_number: String,                 // 伝票番号
    pub date: Option<chrono::NaiveDate>,     // 日付
    pub material_type: Option<String>,       // 品名 (ASガラ, CONガラ, etc.)
    pub weight_tons: f64,                    // 数量(t)
    pub cumulative_tons: Option<f64>,        // 累計(t)
    pub delivery_count: Option<u32>,         // 納入回数
    pub vehicle_number: String,              // 車両番号
    pub transport_company: Option<String>,   // 運送会社
    pub site_name: Option<String>,           // 現場
    pub max_capacity: Option<f64>,           // 最大積載量(t)
    pub is_overloaded: bool,                 // 超過フラグ
}

impl WeighingSlip {
    #[allow(dead_code)]
    pub fn check_overload(&self) -> bool {
        if let Some(max) = self.max_capacity {
            self.weight_tons > max
        } else {
            false
        }
    }
}
