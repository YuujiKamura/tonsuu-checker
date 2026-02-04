//! Overload checking service
//!
//! This module provides functionality to check if vehicles are overloaded
//! by combining weighing slip data with vehicle master data.

use serde::{Deserialize, Serialize};

/// Weighing slip data (typically loaded from CSV)
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct WeighingSlip {
    /// Slip number or ID
    pub slip_no: String,
    /// License plate number
    pub license_plate: String,
    /// Net weight in tonnes
    pub net_weight_tons: f64,
    /// Date of weighing (optional)
    #[serde(default)]
    pub date: Option<String>,
    /// Material type (optional)
    #[serde(default)]
    pub material_type: Option<String>,
}

/// Vehicle master data
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct VehicleMaster {
    /// License plate number
    pub license_plate: String,
    /// Vehicle name/model
    pub name: String,
    /// Maximum payload capacity in tonnes
    pub max_capacity: f64,
    /// Company name (optional)
    #[serde(default)]
    pub company: Option<String>,
}

/// Result of overload check for a single slip
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct OverloadCheckResult {
    /// Original weighing slip
    pub slip: WeighingSlip,
    /// Matched vehicle from master (if found)
    pub vehicle: Option<VehicleMaster>,
    /// Whether the load exceeds the vehicle's max capacity
    pub is_overloaded: bool,
    /// How much over the limit (in tonnes), None if not overloaded or no vehicle match
    pub excess_tons: Option<f64>,
    /// Load ratio as percentage (net_weight / max_capacity * 100)
    pub load_ratio_percent: Option<f64>,
}

/// Check for overloads by matching weighing slips with vehicle master data
///
/// For each slip, attempts to find a matching vehicle by license plate.
/// If found, checks if the net weight exceeds the vehicle's max capacity.
///
/// # Arguments
/// * `slips` - List of weighing slips
/// * `vehicle_master` - List of registered vehicles with their max capacities
///
/// # Returns
/// Vector of check results, one for each input slip
pub fn check_overloads(
    slips: &[WeighingSlip],
    vehicle_master: &[VehicleMaster],
) -> Vec<OverloadCheckResult> {
    slips
        .iter()
        .map(|slip| {
            // Find matching vehicle by license plate
            let vehicle = find_vehicle_by_plate(&slip.license_plate, vehicle_master);

            // Calculate overload status
            let (is_overloaded, excess_tons, load_ratio_percent) = match &vehicle {
                Some(v) => {
                    let excess = slip.net_weight_tons - v.max_capacity;
                    let ratio = (slip.net_weight_tons / v.max_capacity) * 100.0;
                    (excess > 0.0, if excess > 0.0 { Some(excess) } else { None }, Some(ratio))
                }
                None => (false, None, None),
            };

            OverloadCheckResult {
                slip: slip.clone(),
                vehicle,
                is_overloaded,
                excess_tons,
                load_ratio_percent,
            }
        })
        .collect()
}

/// Find a vehicle by license plate with fuzzy matching
fn find_vehicle_by_plate(plate: &str, vehicles: &[VehicleMaster]) -> Option<VehicleMaster> {
    // Normalize the plate for comparison
    let normalized_plate = normalize_plate(plate);

    // Try exact normalized match first
    for vehicle in vehicles {
        if normalize_plate(&vehicle.license_plate) == normalized_plate {
            return Some(vehicle.clone());
        }
    }

    // Try matching by last 4 digits only
    let plate_digits: String = normalized_plate.chars().filter(|c| c.is_ascii_digit()).collect();
    if plate_digits.len() >= 4 {
        let plate_last4 = &plate_digits[plate_digits.len() - 4..];

        for vehicle in vehicles {
            let v_normalized = normalize_plate(&vehicle.license_plate);
            let v_digits: String = v_normalized.chars().filter(|c| c.is_ascii_digit()).collect();
            if v_digits.len() >= 4 {
                let v_last4 = &v_digits[v_digits.len() - 4..];
                if plate_last4 == v_last4 {
                    return Some(vehicle.clone());
                }
            }
        }
    }

    None
}

/// Normalize a license plate string for comparison
fn normalize_plate(plate: &str) -> String {
    plate
        .replace(' ', "")
        .replace('\u{3000}', "") // Full-width space
        .replace('-', "")
        .replace('ー', "") // Full-width hyphen
        .to_lowercase()
}

/// Generate a summary report of overload check results
///
/// # Arguments
/// * `results` - Results from `check_overloads`
///
/// # Returns
/// A formatted string report containing:
/// - Total number of slips
/// - Number of overloaded entries
/// - Number of unmatched vehicles
/// - List of overloaded entries with details
pub fn generate_overload_report(results: &[OverloadCheckResult]) -> String {
    let total = results.len();
    let overloaded_count = results.iter().filter(|r| r.is_overloaded).count();
    let unmatched_count = results.iter().filter(|r| r.vehicle.is_none()).count();
    let matched_count = total - unmatched_count;

    let mut report = String::new();

    // Header
    report.push_str("==================================================\n");
    report.push_str("              過積載チェックレポート               \n");
    report.push_str("              Overload Check Report                \n");
    report.push_str("==================================================\n\n");

    // Summary
    report.push_str("【サマリー / Summary】\n");
    report.push_str(&format!("  総伝票数 / Total slips:         {}\n", total));
    report.push_str(&format!("  車両照合成功 / Matched:         {}\n", matched_count));
    report.push_str(&format!("  車両未登録 / Unmatched:         {}\n", unmatched_count));
    report.push_str(&format!("  過積載件数 / Overloaded:        {}\n", overloaded_count));

    if matched_count > 0 {
        let overload_rate = (overloaded_count as f64 / matched_count as f64) * 100.0;
        report.push_str(&format!("  過積載率 / Overload rate:       {:.1}%\n", overload_rate));
    }
    report.push('\n');

    // Overloaded entries list
    if overloaded_count > 0 {
        report.push_str("【過積載一覧 / Overloaded Entries】\n");
        report.push_str("-".repeat(70).as_str());
        report.push('\n');
        report.push_str(&format!(
            "{:<12} {:<16} {:>8} {:>8} {:>8} {:>8}\n",
            "伝票No", "ナンバー", "積載量", "上限", "超過", "積載率"
        ));
        report.push_str(&format!(
            "{:<12} {:<16} {:>8} {:>8} {:>8} {:>8}\n",
            "Slip No", "License", "Weight", "Limit", "Excess", "Ratio"
        ));
        report.push_str("-".repeat(70).as_str());
        report.push('\n');

        for result in results.iter().filter(|r| r.is_overloaded) {
            let vehicle = result.vehicle.as_ref().unwrap(); // Safe: overloaded implies vehicle exists
            let excess = result.excess_tons.unwrap_or(0.0);
            let ratio = result.load_ratio_percent.unwrap_or(0.0);

            report.push_str(&format!(
                "{:<12} {:<16} {:>7.2}t {:>7.2}t {:>+7.2}t {:>7.1}%\n",
                truncate_str(&result.slip.slip_no, 11),
                truncate_str(&result.slip.license_plate, 15),
                result.slip.net_weight_tons,
                vehicle.max_capacity,
                excess,
                ratio
            ));
        }
        report.push('\n');
    } else {
        report.push_str("【過積載なし / No Overloaded Entries】\n");
        report.push_str("  全ての照合済み伝票は積載量制限内です。\n");
        report.push_str("  All matched slips are within weight limits.\n\n");
    }

    // Unmatched entries list
    if unmatched_count > 0 {
        report.push_str("【車両未登録一覧 / Unmatched Vehicles】\n");
        report.push_str("-".repeat(50).as_str());
        report.push('\n');
        report.push_str(&format!(
            "{:<12} {:<20} {:>10}\n",
            "伝票No", "ナンバー", "積載量"
        ));
        report.push_str("-".repeat(50).as_str());
        report.push('\n');

        for result in results.iter().filter(|r| r.vehicle.is_none()) {
            report.push_str(&format!(
                "{:<12} {:<20} {:>9.2}t\n",
                truncate_str(&result.slip.slip_no, 11),
                truncate_str(&result.slip.license_plate, 19),
                result.slip.net_weight_tons
            ));
        }
        report.push('\n');
    }

    report.push_str("==================================================\n");

    report
}

/// Truncate a string to max length, adding "..." if truncated
fn truncate_str(s: &str, max_len: usize) -> String {
    if s.chars().count() > max_len {
        let truncated: String = s.chars().take(max_len.saturating_sub(2)).collect();
        format!("{}..", truncated)
    } else {
        s.to_string()
    }
}

/// Load weighing slips from CSV file
///
/// Expected CSV format:
/// slip_no,license_plate,net_weight_tons,date,material_type
///
/// # Arguments
/// * `path` - Path to CSV file
///
/// # Returns
/// Vector of weighing slips or error
pub fn load_slips_from_csv(path: &std::path::Path) -> Result<Vec<WeighingSlip>, String> {
    let content = std::fs::read_to_string(path)
        .map_err(|e| format!("Failed to read CSV file: {}", e))?;

    let mut slips = Vec::new();
    let mut lines = content.lines();

    // Skip header if present
    let first_line = lines.next().ok_or("CSV file is empty")?;
    let headers: Vec<&str> = first_line.split(',').map(|s| s.trim()).collect();

    // Check if first line is a header
    let is_header = headers.iter().any(|h| {
        h.to_lowercase().contains("slip")
            || h.to_lowercase().contains("plate")
            || h.to_lowercase().contains("weight")
            || h.contains("伝票")
            || h.contains("ナンバー")
            || h.contains("重量")
    });

    // If first line is data, process it
    if !is_header {
        if let Some(slip) = parse_csv_line(first_line, &headers) {
            slips.push(slip);
        }
    }

    // Process remaining lines
    for line in lines {
        let line = line.trim();
        if line.is_empty() {
            continue;
        }
        if let Some(slip) = parse_csv_line(line, &headers) {
            slips.push(slip);
        }
    }

    Ok(slips)
}

/// Parse a single CSV line into a WeighingSlip
fn parse_csv_line(line: &str, _headers: &[&str]) -> Option<WeighingSlip> {
    let fields: Vec<&str> = line.split(',').map(|s| s.trim()).collect();

    if fields.len() < 3 {
        return None;
    }

    let slip_no = fields.first()?.to_string();
    let license_plate = fields.get(1)?.to_string();
    let net_weight_tons: f64 = fields.get(2)?.parse().ok()?;

    let date = fields.get(3).map(|s| s.to_string());
    let material_type = fields.get(4).map(|s| s.to_string());

    Some(WeighingSlip {
        slip_no,
        license_plate,
        net_weight_tons,
        date,
        material_type,
    })
}

/// Load vehicle master from CSV file
///
/// Expected CSV format:
/// license_plate,name,max_capacity,company
///
/// # Arguments
/// * `path` - Path to CSV file
///
/// # Returns
/// Vector of vehicle master records or error
pub fn load_vehicles_from_csv(path: &std::path::Path) -> Result<Vec<VehicleMaster>, String> {
    let content = std::fs::read_to_string(path)
        .map_err(|e| format!("Failed to read CSV file: {}", e))?;

    let mut vehicles = Vec::new();
    let mut lines = content.lines();

    // Skip header if present
    let first_line = lines.next().ok_or("CSV file is empty")?;
    let headers: Vec<&str> = first_line.split(',').map(|s| s.trim()).collect();

    // Check if first line is a header
    let is_header = headers.iter().any(|h| {
        h.to_lowercase().contains("plate")
            || h.to_lowercase().contains("name")
            || h.to_lowercase().contains("capacity")
            || h.contains("ナンバー")
            || h.contains("車名")
            || h.contains("積載")
    });

    // If first line is data, process it
    if !is_header {
        if let Some(vehicle) = parse_vehicle_csv_line(first_line) {
            vehicles.push(vehicle);
        }
    }

    // Process remaining lines
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

/// Parse a single CSV line into a VehicleMaster
fn parse_vehicle_csv_line(line: &str) -> Option<VehicleMaster> {
    let fields: Vec<&str> = line.split(',').map(|s| s.trim()).collect();

    if fields.len() < 3 {
        return None;
    }

    let license_plate = fields.first()?.to_string();
    let name = fields.get(1)?.to_string();
    let max_capacity: f64 = fields.get(2)?.parse().ok()?;
    let company = fields.get(3).map(|s| s.to_string());

    Some(VehicleMaster {
        license_plate,
        name,
        max_capacity,
        company,
    })
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_no_overload() {
        let slips = vec![WeighingSlip {
            slip_no: "001".to_string(),
            license_plate: "熊本 100 あ 1234".to_string(),
            net_weight_tons: 8.5,
            date: None,
            material_type: None,
        }];

        let vehicles = vec![VehicleMaster {
            license_plate: "熊本 100 あ 1234".to_string(),
            name: "10t truck".to_string(),
            max_capacity: 10.0,
            company: None,
        }];

        let results = check_overloads(&slips, &vehicles);
        assert_eq!(results.len(), 1);
        assert!(!results[0].is_overloaded);
        assert!(results[0].excess_tons.is_none());
        assert!(results[0].vehicle.is_some());
    }

    #[test]
    fn test_overload() {
        let slips = vec![WeighingSlip {
            slip_no: "002".to_string(),
            license_plate: "熊本 100 あ 1234".to_string(),
            net_weight_tons: 12.5,
            date: None,
            material_type: None,
        }];

        let vehicles = vec![VehicleMaster {
            license_plate: "熊本 100 あ 1234".to_string(),
            name: "10t truck".to_string(),
            max_capacity: 10.0,
            company: None,
        }];

        let results = check_overloads(&slips, &vehicles);
        assert_eq!(results.len(), 1);
        assert!(results[0].is_overloaded);
        assert!((results[0].excess_tons.unwrap() - 2.5).abs() < 0.01);
    }

    #[test]
    fn test_unmatched_vehicle() {
        let slips = vec![WeighingSlip {
            slip_no: "003".to_string(),
            license_plate: "福岡 200 い 5678".to_string(),
            net_weight_tons: 8.0,
            date: None,
            material_type: None,
        }];

        let vehicles = vec![VehicleMaster {
            license_plate: "熊本 100 あ 1234".to_string(),
            name: "10t truck".to_string(),
            max_capacity: 10.0,
            company: None,
        }];

        let results = check_overloads(&slips, &vehicles);
        assert_eq!(results.len(), 1);
        assert!(!results[0].is_overloaded);
        assert!(results[0].vehicle.is_none());
    }

    #[test]
    fn test_fuzzy_plate_match() {
        let slips = vec![WeighingSlip {
            slip_no: "004".to_string(),
            license_plate: "熊本100あ1234".to_string(), // No spaces
            net_weight_tons: 8.5,
            date: None,
            material_type: None,
        }];

        let vehicles = vec![VehicleMaster {
            license_plate: "熊本 100 あ 1234".to_string(), // With spaces
            name: "10t truck".to_string(),
            max_capacity: 10.0,
            company: None,
        }];

        let results = check_overloads(&slips, &vehicles);
        assert!(results[0].vehicle.is_some());
    }

    #[test]
    fn test_generate_report() {
        let slips = vec![
            WeighingSlip {
                slip_no: "001".to_string(),
                license_plate: "熊本 100 あ 1234".to_string(),
                net_weight_tons: 12.5,
                date: None,
                material_type: None,
            },
            WeighingSlip {
                slip_no: "002".to_string(),
                license_plate: "熊本 100 あ 1234".to_string(),
                net_weight_tons: 8.0,
                date: None,
                material_type: None,
            },
        ];

        let vehicles = vec![VehicleMaster {
            license_plate: "熊本 100 あ 1234".to_string(),
            name: "10t truck".to_string(),
            max_capacity: 10.0,
            company: None,
        }];

        let results = check_overloads(&slips, &vehicles);
        let report = generate_overload_report(&results);

        assert!(report.contains("過積載チェックレポート"));
        assert!(report.contains("2")); // Total slips
        assert!(report.contains("1")); // One overloaded
    }
}
