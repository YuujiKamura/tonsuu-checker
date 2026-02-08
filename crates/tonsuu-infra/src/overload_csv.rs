//! CSV loaders for overload checking (simple format)

use tonsuu_domain::model::{VehicleMaster, WeighingSlip};

/// Load weighing slips from a simple CSV file
///
/// Expected columns (no header required):
/// slip_no, license_plate, net_weight_tons, [date], [material_type]
pub fn load_slips_from_csv(path: &std::path::Path) -> Result<Vec<WeighingSlip>, String> {
    let content = std::fs::read_to_string(path)
        .map_err(|e| format!("Failed to read CSV file: {}", e))?;
    let mut slips = Vec::new();
    let mut lines = content.lines();
    let first_line = lines.next().ok_or("CSV file is empty")?;
    let headers: Vec<&str> = first_line.split(',').map(|s| s.trim()).collect();
    let is_header = headers.iter().any(|h| {
        h.to_lowercase().contains("slip")
            || h.to_lowercase().contains("plate")
            || h.to_lowercase().contains("weight")
            || h.contains("伝票")
            || h.contains("ナンバー")
            || h.contains("重量")
    });
    if !is_header {
        if let Some(slip) = parse_csv_line(first_line) {
            slips.push(slip);
        }
    }
    for line in lines {
        let line = line.trim();
        if line.is_empty() {
            continue;
        }
        if let Some(slip) = parse_csv_line(line) {
            slips.push(slip);
        }
    }
    Ok(slips)
}

fn parse_csv_line(line: &str) -> Option<WeighingSlip> {
    let fields: Vec<&str> = line.split(',').map(|s| s.trim()).collect();
    if fields.len() < 3 {
        return None;
    }
    let slip_number = fields.first()?.to_string();
    let vehicle_number = fields.get(1)?.to_string();
    let weight_tons: f64 = fields.get(2)?.parse().ok()?;
    let date = fields.get(3).and_then(|s| parse_optional_date(s));
    let material_type = fields.get(4).map(|s| s.to_string()).filter(|s| !s.is_empty());

    Some(WeighingSlip {
        slip_number,
        date,
        material_type,
        weight_tons,
        cumulative_tons: None,
        delivery_count: None,
        vehicle_number,
        transport_company: None,
        site_name: None,
        max_capacity: None,
        is_overloaded: false,
    })
}

fn parse_optional_date(s: &str) -> Option<chrono::NaiveDate> {
    if s.trim().is_empty() {
        return None;
    }
    let formats = ["%Y/%m/%d", "%Y-%m-%d", "%Y年%m月%d日"];
    for fmt in formats {
        if let Ok(date) = chrono::NaiveDate::parse_from_str(s, fmt) {
            return Some(date);
        }
    }
    None
}

/// Load vehicle master data from a simple CSV file
///
/// Expected columns (no header required):
/// license_plate, name, max_capacity, [company]
pub fn load_vehicles_from_csv(path: &std::path::Path) -> Result<Vec<VehicleMaster>, String> {
    let content = std::fs::read_to_string(path)
        .map_err(|e| format!("Failed to read CSV file: {}", e))?;
    let mut vehicles = Vec::new();
    let mut lines = content.lines();
    let first_line = lines.next().ok_or("CSV file is empty")?;
    let headers: Vec<&str> = first_line.split(',').map(|s| s.trim()).collect();
    let is_header = headers.iter().any(|h| {
        h.to_lowercase().contains("plate")
            || h.to_lowercase().contains("name")
            || h.to_lowercase().contains("capacity")
            || h.contains("ナンバー")
            || h.contains("車名")
            || h.contains("積載")
    });
    if !is_header {
        if let Some(vehicle) = parse_vehicle_csv_line(first_line) {
            vehicles.push(vehicle);
        }
    }
    for line in lines {
        let line = line.trim();
        if line.is_empty() {
            continue;
        }
        if let Some(vehicle) = parse_vehicle_csv_line(line) {
            vehicles.push(vehicle);
        }
    }
    Ok(vehicles)
}

fn parse_vehicle_csv_line(line: &str) -> Option<VehicleMaster> {
    let fields: Vec<&str> = line.split(',').map(|s| s.trim()).collect();
    if fields.len() < 3 {
        return None;
    }
    let vehicle_number = fields.first()?.to_string();
    let name = fields.get(1).map(|s| s.to_string()).unwrap_or_default();
    let max_capacity_tons: f64 = fields.get(2)?.parse().ok()?;
    let company = fields.get(3).map(|s| s.to_string()).unwrap_or_default();

    Some(VehicleMaster {
        vehicle_number,
        max_capacity_tons,
        transport_company: if !company.is_empty() { company } else { name },
        truck_type: None,
    })
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_parse_vehicle_csv_line() {
        let line = "熊本 100 あ 1234,10t truck,10.0,松尾運搬";
        let vehicle = parse_vehicle_csv_line(line).unwrap();
        assert_eq!(vehicle.vehicle_number, "熊本 100 あ 1234");
        assert_eq!(vehicle.max_capacity_tons, 10.0);
        assert_eq!(vehicle.transport_company, "松尾運搬");
    }

    #[test]
    fn test_parse_slip_csv_line() {
        let line = "001,熊本 100 あ 1234,12.5,2024/01/15,土砂";
        let slip = parse_csv_line(line).unwrap();
        assert_eq!(slip.slip_number, "001");
        assert_eq!(slip.vehicle_number, "熊本 100 あ 1234");
        assert!((slip.weight_tons - 12.5).abs() < 0.01);
        assert_eq!(slip.material_type.as_deref(), Some("土砂"));
        assert!(slip.date.is_some());
    }
}
