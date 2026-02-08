//! Overload checking service

use serde::{Deserialize, Serialize};

use crate::model::{VehicleMaster, WeighingSlip};

/// Result of overload check for a single slip
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct OverloadCheckResult {
    pub slip: WeighingSlip,
    pub vehicle: Option<VehicleMaster>,
    pub is_overloaded: bool,
    pub excess_tons: Option<f64>,
    pub load_ratio_percent: Option<f64>,
}

pub fn check_overloads(
    slips: &[WeighingSlip],
    vehicle_master: &[VehicleMaster],
) -> Vec<OverloadCheckResult> {
    slips
        .iter()
        .map(|slip| {
            let vehicle = find_vehicle_by_plate(&slip.vehicle_number, vehicle_master);
            let (is_overloaded, excess_tons, load_ratio_percent) = match &vehicle {
                Some(v) => {
                    let excess = slip.weight_tons - v.max_capacity_tons;
                    let ratio = (slip.weight_tons / v.max_capacity_tons) * 100.0;
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

fn find_vehicle_by_plate(plate: &str, vehicles: &[VehicleMaster]) -> Option<VehicleMaster> {
    let normalized_plate = normalize_plate(plate);
    for vehicle in vehicles {
        if normalize_plate(&vehicle.vehicle_number) == normalized_plate {
            return Some(vehicle.clone());
        }
    }
    let plate_digits: String = normalized_plate.chars().filter(|c| c.is_ascii_digit()).collect();
    if plate_digits.len() >= 4 {
        let plate_last4 = &plate_digits[plate_digits.len() - 4..];
        for vehicle in vehicles {
            let v_normalized = normalize_plate(&vehicle.vehicle_number);
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

fn normalize_plate(plate: &str) -> String {
    plate
        .replace(' ', "")
        .replace('\u{3000}', "")
        .replace('-', "")
        .replace('ー', "")
        .to_lowercase()
}

pub fn generate_overload_report(results: &[OverloadCheckResult]) -> String {
    let total = results.len();
    let overloaded_count = results.iter().filter(|r| r.is_overloaded).count();
    let unmatched_count = results.iter().filter(|r| r.vehicle.is_none()).count();
    let matched_count = total - unmatched_count;

    let mut report = String::new();
    report.push_str("==================================================\n");
    report.push_str("              過積載チェックレポート               \n");
    report.push_str("              Overload Check Report                \n");
    report.push_str("==================================================\n\n");
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
            let vehicle = result.vehicle.as_ref().unwrap();
            let excess = result.excess_tons.unwrap_or(0.0);
            let ratio = result.load_ratio_percent.unwrap_or(0.0);
            report.push_str(&format!(
                "{:<12} {:<16} {:>7.2}t {:>7.2}t {:>+7.2}t {:>7.1}%\n",
                truncate_str(&result.slip.slip_number, 11),
                truncate_str(&result.slip.vehicle_number, 15),
                result.slip.weight_tons,
                vehicle.max_capacity_tons,
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
                truncate_str(&result.slip.slip_number, 11),
                truncate_str(&result.slip.vehicle_number, 19),
                result.slip.weight_tons
            ));
        }
        report.push('\n');
    }

    report.push_str("==================================================\n");
    report
}

fn truncate_str(s: &str, max_len: usize) -> String {
    if s.chars().count() > max_len {
        let truncated: String = s.chars().take(max_len.saturating_sub(2)).collect();
        format!("{}..", truncated)
    } else {
        s.to_string()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_no_overload() {
        let slips = vec![WeighingSlip {
            slip_number: "001".to_string(),
            vehicle_number: "熊本 100 あ 1234".to_string(),
            weight_tons: 8.5,
            date: None,
            material_type: None,
            cumulative_tons: None,
            delivery_count: None,
            transport_company: None,
            site_name: None,
            max_capacity: None,
            is_overloaded: false,
        }];
        let vehicles = vec![VehicleMaster {
            vehicle_number: "熊本 100 あ 1234".to_string(),
            max_capacity_tons: 10.0,
            transport_company: "".to_string(),
            truck_type: None,
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
            slip_number: "002".to_string(),
            vehicle_number: "熊本 100 あ 1234".to_string(),
            weight_tons: 12.5,
            date: None,
            material_type: None,
            cumulative_tons: None,
            delivery_count: None,
            transport_company: None,
            site_name: None,
            max_capacity: None,
            is_overloaded: false,
        }];
        let vehicles = vec![VehicleMaster {
            vehicle_number: "熊本 100 あ 1234".to_string(),
            max_capacity_tons: 10.0,
            transport_company: "".to_string(),
            truck_type: None,
        }];
        let results = check_overloads(&slips, &vehicles);
        assert_eq!(results.len(), 1);
        assert!(results[0].is_overloaded);
        assert!((results[0].excess_tons.unwrap() - 2.5).abs() < 0.01);
    }

    #[test]
    fn test_unmatched_vehicle() {
        let slips = vec![WeighingSlip {
            slip_number: "003".to_string(),
            vehicle_number: "福岡 200 い 5678".to_string(),
            weight_tons: 8.0,
            date: None,
            material_type: None,
            cumulative_tons: None,
            delivery_count: None,
            transport_company: None,
            site_name: None,
            max_capacity: None,
            is_overloaded: false,
        }];
        let vehicles = vec![VehicleMaster {
            vehicle_number: "熊本 100 あ 1234".to_string(),
            max_capacity_tons: 10.0,
            transport_company: "".to_string(),
            truck_type: None,
        }];
        let results = check_overloads(&slips, &vehicles);
        assert_eq!(results.len(), 1);
        assert!(!results[0].is_overloaded);
        assert!(results[0].vehicle.is_none());
    }

    #[test]
    fn test_fuzzy_plate_match() {
        let slips = vec![WeighingSlip {
            slip_number: "004".to_string(),
            vehicle_number: "熊本100あ1234".to_string(),
            weight_tons: 8.5,
            date: None,
            material_type: None,
            cumulative_tons: None,
            delivery_count: None,
            transport_company: None,
            site_name: None,
            max_capacity: None,
            is_overloaded: false,
        }];
        let vehicles = vec![VehicleMaster {
            vehicle_number: "熊本 100 あ 1234".to_string(),
            max_capacity_tons: 10.0,
            transport_company: "".to_string(),
            truck_type: None,
        }];
        let results = check_overloads(&slips, &vehicles);
        assert!(results[0].vehicle.is_some());
    }

    #[test]
    fn test_generate_report() {
        let slips = vec![
            WeighingSlip {
                slip_number: "001".to_string(),
                vehicle_number: "熊本 100 あ 1234".to_string(),
                weight_tons: 12.5,
                date: None,
                material_type: None,
                cumulative_tons: None,
                delivery_count: None,
                transport_company: None,
                site_name: None,
                max_capacity: None,
                is_overloaded: false,
            },
            WeighingSlip {
                slip_number: "002".to_string(),
                vehicle_number: "熊本 100 あ 1234".to_string(),
                weight_tons: 8.0,
                date: None,
                material_type: None,
                cumulative_tons: None,
                delivery_count: None,
                transport_company: None,
                site_name: None,
                max_capacity: None,
                is_overloaded: false,
            },
        ];
        let vehicles = vec![VehicleMaster {
            vehicle_number: "熊本 100 あ 1234".to_string(),
            max_capacity_tons: 10.0,
            transport_company: "".to_string(),
            truck_type: None,
        }];
        let results = check_overloads(&slips, &vehicles);
        let report = generate_overload_report(&results);
        assert!(report.contains("過積載チェックレポート"));
        assert!(report.contains("2"));
        assert!(report.contains("1"));
    }
}
