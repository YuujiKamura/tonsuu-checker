//! CSV loader for weighing slips (計量伝票)
//!
//! Handles CP932 (Shift-JIS) encoded CSV files commonly used in Japanese business systems.

#![allow(dead_code)]

use std::fs::File;
use std::io::Read;
use std::path::Path;

use chrono::NaiveDate;
use encoding_rs::SHIFT_JIS;
use thiserror::Error;

use crate::domain::model::WeighingSlip;

#[derive(Error, Debug)]
pub enum CsvLoaderError {
    #[error("Failed to read file: {0}")]
    IoError(#[from] std::io::Error),

    #[error("Failed to parse CSV: {0}")]
    CsvError(#[from] csv::Error),

    #[error("Invalid date format in row {row}: {value}")]
    InvalidDate { row: usize, value: String },

    #[error("Invalid number format in row {row}, column {column}: {value}")]
    InvalidNumber {
        row: usize,
        column: String,
        value: String,
    },

    #[error("Missing required column: {0}")]
    MissingColumn(String),
}

/// Load weighing slips from a CP932 encoded CSV file
///
/// Expected CSV header:
/// 伝票番号,日付,品名,数量(t),累計(t),納入回数,車両番号,運送会社,現場,最大積載量(t),超過
pub fn load_weighing_slips<P: AsRef<Path>>(
    path: P,
) -> Result<Vec<WeighingSlip>, CsvLoaderError> {
    // Read file as bytes
    let mut file = File::open(path)?;
    let mut bytes = Vec::new();
    file.read_to_end(&mut bytes)?;

    // Decode from CP932 (Shift-JIS) to UTF-8
    let (decoded, _, had_errors) = SHIFT_JIS.decode(&bytes);
    if had_errors {
        // Log warning but continue - some characters might not decode perfectly
        eprintln!("Warning: Some characters could not be decoded from CP932");
    }

    // Parse CSV
    let mut reader = csv::ReaderBuilder::new()
        .has_headers(true)
        .flexible(true)
        .trim(csv::Trim::All)
        .from_reader(decoded.as_bytes());

    let headers = reader.headers()?.clone();
    validate_headers(&headers)?;

    let mut slips = Vec::new();
    for (row_idx, result) in reader.records().enumerate() {
        let record = result?;
        let row_num = row_idx + 2; // +2 because row_idx is 0-based and header is row 1

        let slip = parse_record(&record, row_num)?;
        slips.push(slip);
    }

    Ok(slips)
}

fn validate_headers(headers: &csv::StringRecord) -> Result<(), CsvLoaderError> {
    let required = [
        "伝票番号",
        "日付",
        "品名",
        "数量(t)",
        "累計(t)",
        "納入回数",
        "車両番号",
        "運送会社",
        "現場",
    ];

    for col in required {
        if !headers.iter().any(|h| h == col) {
            return Err(CsvLoaderError::MissingColumn(col.to_string()));
        }
    }

    Ok(())
}

fn parse_record(
    record: &csv::StringRecord,
    row_num: usize,
) -> Result<WeighingSlip, CsvLoaderError> {
    let slip_number = record.get(0).unwrap_or("").to_string();

    let date_str = record.get(1).unwrap_or("");
    let date = parse_date(date_str, row_num)?;

    let material_type = record.get(2).unwrap_or("").to_string();

    let weight_tons = parse_f64(record.get(3).unwrap_or("0"), row_num, "数量(t)")?;
    let cumulative_tons = parse_f64(record.get(4).unwrap_or("0"), row_num, "累計(t)")?;
    let delivery_count = parse_u32(record.get(5).unwrap_or("0"), row_num, "納入回数")?;

    let vehicle_number = record.get(6).unwrap_or("").to_string();
    let transport_company = record.get(7).unwrap_or("").to_string();
    let site_name = record.get(8).unwrap_or("").to_string();

    let max_capacity = record
        .get(9)
        .and_then(|s| if s.is_empty() { None } else { Some(s) })
        .map(|s| parse_f64(s, row_num, "最大積載量(t)"))
        .transpose()?;

    let is_overloaded_str = record.get(10).unwrap_or("");
    let is_overloaded = parse_overload_flag(is_overloaded_str);

    let mut slip = WeighingSlip {
        slip_number,
        date,
        material_type,
        weight_tons,
        cumulative_tons,
        delivery_count,
        vehicle_number,
        transport_company,
        site_name,
        max_capacity,
        is_overloaded,
    };

    // Recompute overload flag if max_capacity is available
    if slip.max_capacity.is_some() {
        slip.is_overloaded = slip.check_overload();
    }

    Ok(slip)
}

fn parse_date(s: &str, row: usize) -> Result<NaiveDate, CsvLoaderError> {
    // Try common Japanese date formats
    // Format: YYYY/MM/DD or YYYY-MM-DD
    let formats = ["%Y/%m/%d", "%Y-%m-%d", "%Y年%m月%d日"];

    for fmt in formats {
        if let Ok(date) = NaiveDate::parse_from_str(s, fmt) {
            return Ok(date);
        }
    }

    Err(CsvLoaderError::InvalidDate {
        row,
        value: s.to_string(),
    })
}

fn parse_f64(s: &str, row: usize, column: &str) -> Result<f64, CsvLoaderError> {
    let cleaned = s.trim().replace(',', "");
    if cleaned.is_empty() {
        return Ok(0.0);
    }

    cleaned.parse().map_err(|_| CsvLoaderError::InvalidNumber {
        row,
        column: column.to_string(),
        value: s.to_string(),
    })
}

fn parse_u32(s: &str, row: usize, column: &str) -> Result<u32, CsvLoaderError> {
    let cleaned = s.trim().replace(',', "");
    if cleaned.is_empty() {
        return Ok(0);
    }

    cleaned.parse().map_err(|_| CsvLoaderError::InvalidNumber {
        row,
        column: column.to_string(),
        value: s.to_string(),
    })
}

fn parse_overload_flag(s: &str) -> bool {
    let s = s.trim().to_lowercase();
    matches!(
        s.as_str(),
        "1" | "true" | "yes" | "○" | "超過" | "あり" | "有"
    )
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_parse_date_slash() {
        let date = parse_date("2024/01/15", 1).unwrap();
        assert_eq!(date, NaiveDate::from_ymd_opt(2024, 1, 15).unwrap());
    }

    #[test]
    fn test_parse_date_hyphen() {
        let date = parse_date("2024-01-15", 1).unwrap();
        assert_eq!(date, NaiveDate::from_ymd_opt(2024, 1, 15).unwrap());
    }

    #[test]
    fn test_parse_f64_with_comma() {
        let val = parse_f64("1,234.56", 1, "test").unwrap();
        assert!((val - 1234.56).abs() < 0.001);
    }

    #[test]
    fn test_parse_overload_flag() {
        assert!(parse_overload_flag("○"));
        assert!(parse_overload_flag("1"));
        assert!(parse_overload_flag("超過"));
        assert!(!parse_overload_flag(""));
        assert!(!parse_overload_flag("0"));
    }
}
