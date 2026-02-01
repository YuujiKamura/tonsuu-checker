//! Excel export functionality

use crate::error::{Error, Result};
use crate::types::BatchResults;
use rust_xlsxwriter::{Format, Workbook, Worksheet};
use std::path::Path;

/// Export batch results to Excel file
pub fn export_to_excel(results: &BatchResults, output_path: &Path) -> Result<()> {
    let mut workbook = Workbook::new();

    // Add summary sheet
    let summary_sheet = workbook.add_worksheet();
    write_summary_sheet(summary_sheet, results)?;

    // Add details sheet
    let details_sheet = workbook.add_worksheet();
    write_details_sheet(details_sheet, results)?;

    // Save workbook
    workbook
        .save(output_path)
        .map_err(|e| Error::Excel(e.to_string()))?;

    Ok(())
}

fn write_summary_sheet(sheet: &mut Worksheet, results: &BatchResults) -> Result<()> {
    sheet
        .set_name("Summary")
        .map_err(|e| Error::Excel(e.to_string()))?;

    // Header format
    let header_format = Format::new().set_bold();

    // Write headers
    sheet
        .write_string_with_format(0, 0, "Tonnage Checker Analysis Report", &header_format)
        .map_err(|e| Error::Excel(e.to_string()))?;

    sheet
        .write_string(2, 0, "Analysis Date:")
        .map_err(|e| Error::Excel(e.to_string()))?;
    sheet
        .write_string(2, 1, &results.started_at.to_rfc3339())
        .map_err(|e| Error::Excel(e.to_string()))?;

    sheet
        .write_string(3, 0, "Total Images:")
        .map_err(|e| Error::Excel(e.to_string()))?;
    sheet
        .write_number(3, 1, results.total_processed as f64)
        .map_err(|e| Error::Excel(e.to_string()))?;

    sheet
        .write_string(4, 0, "Successful:")
        .map_err(|e| Error::Excel(e.to_string()))?;
    sheet
        .write_number(4, 1, results.successful as f64)
        .map_err(|e| Error::Excel(e.to_string()))?;

    sheet
        .write_string(5, 0, "Failed:")
        .map_err(|e| Error::Excel(e.to_string()))?;
    sheet
        .write_number(5, 1, results.failed as f64)
        .map_err(|e| Error::Excel(e.to_string()))?;

    // Grade distribution
    sheet
        .write_string_with_format(7, 0, "Grade Distribution", &header_format)
        .map_err(|e| Error::Excel(e.to_string()))?;

    let mut grade_counts = std::collections::HashMap::new();
    for entry in &results.entries {
        if let Some(grade) = entry.grade {
            *grade_counts.entry(grade.label_en().to_string()).or_insert(0) += 1;
        }
    }

    let mut row = 8;
    for (grade, count) in &grade_counts {
        sheet
            .write_string(row, 0, grade)
            .map_err(|e| Error::Excel(e.to_string()))?;
        sheet
            .write_number(row, 1, *count as f64)
            .map_err(|e| Error::Excel(e.to_string()))?;
        row += 1;
    }

    Ok(())
}

fn write_details_sheet(sheet: &mut Worksheet, results: &BatchResults) -> Result<()> {
    sheet
        .set_name("Details")
        .map_err(|e| Error::Excel(e.to_string()))?;

    // Header format
    let header_format = Format::new().set_bold();

    // Write headers
    let headers = [
        "File",
        "Truck Type",
        "Material",
        "Volume (m³)",
        "Tonnage (t)",
        "Max Capacity (t)",
        "Load %",
        "Grade",
        "Confidence",
        "Reasoning",
    ];

    for (col, header) in headers.iter().enumerate() {
        sheet
            .write_string_with_format(0, col as u16, *header, &header_format)
            .map_err(|e| Error::Excel(e.to_string()))?;
    }

    // Write data
    for (row_idx, entry) in results.entries.iter().enumerate() {
        let row = (row_idx + 1) as u32;
        let result = &entry.result;

        // File name
        let filename = Path::new(&entry.image_path)
            .file_name()
            .and_then(|n| n.to_str())
            .unwrap_or(&entry.image_path);
        sheet
            .write_string(row, 0, filename)
            .map_err(|e| Error::Excel(e.to_string()))?;

        // Truck type
        sheet
            .write_string(row, 1, &result.truck_type)
            .map_err(|e| Error::Excel(e.to_string()))?;

        // Material
        sheet
            .write_string(row, 2, &result.material_type)
            .map_err(|e| Error::Excel(e.to_string()))?;

        // Volume
        sheet
            .write_number(row, 3, result.estimated_volume_m3)
            .map_err(|e| Error::Excel(e.to_string()))?;

        // Tonnage
        sheet
            .write_number(row, 4, result.estimated_tonnage)
            .map_err(|e| Error::Excel(e.to_string()))?;

        // Max capacity
        if let Some(max_cap) = result.estimated_max_capacity {
            sheet
                .write_number(row, 5, max_cap)
                .map_err(|e| Error::Excel(e.to_string()))?;

            // Load percentage
            let load_pct = (result.estimated_tonnage / max_cap) * 100.0;
            sheet
                .write_number(row, 6, load_pct)
                .map_err(|e| Error::Excel(e.to_string()))?;
        }

        // Grade
        if let Some(grade) = entry.grade {
            sheet
                .write_string(row, 7, grade.label())
                .map_err(|e| Error::Excel(e.to_string()))?;
        }

        // Confidence
        sheet
            .write_number(row, 8, result.confidence_score)
            .map_err(|e| Error::Excel(e.to_string()))?;

        // Reasoning (truncate for Excel)
        let reasoning = if result.reasoning.len() > 200 {
            format!("{}...", &result.reasoning[..200])
        } else {
            result.reasoning.clone()
        };
        sheet
            .write_string(row, 9, &reasoning)
            .map_err(|e| Error::Excel(e.to_string()))?;
    }

    // Auto-fit columns (approximate)
    sheet
        .set_column_width(0, 30)
        .map_err(|e| Error::Excel(e.to_string()))?;
    sheet
        .set_column_width(1, 12)
        .map_err(|e| Error::Excel(e.to_string()))?;
    sheet
        .set_column_width(2, 12)
        .map_err(|e| Error::Excel(e.to_string()))?;
    sheet
        .set_column_width(9, 50)
        .map_err(|e| Error::Excel(e.to_string()))?;

    Ok(())
}
