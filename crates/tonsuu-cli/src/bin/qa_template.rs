use std::path::PathBuf;

use rust_xlsxwriter::{Format, Workbook};

fn main() -> Result<(), Box<dyn std::error::Error>> {
    let output = PathBuf::from("C:/Users/yuuji/tonsuu-checker/docs/qa/2026-02-08-gui-manual-test-result.xlsx");

    let mut workbook = Workbook::new();
    let sheet = workbook.add_worksheet();
    sheet.set_name("GUI Test Results")?;

    let header = Format::new().set_bold();

    let headers = [
        "Section",
        "Test Item",
        "Steps",
        "Expected",
        "Actual",
        "Status",
        "Notes",
    ];

    for (col, h) in headers.iter().enumerate() {
        sheet.write_string_with_format(0, col as u16, *h, &header)?;
    }

    let rows: Vec<[&str; 7]> = vec![
        ["Launch", "App launches", "Run app", "No crash", "", "", ""],
        ["Tabs", "Tab navigation", "Click each tab", "All tabs render", "", "", ""],
        ["Analyze", "Run analysis", "Select image, run", "Result shown", "", "", ""],
        ["Vehicle", "Add vehicle", "Add & save", "Saved", "", "", ""],
        ["Vehicle", "Edit vehicle", "Edit & save", "Updated", "", "", ""],
        ["Vehicle", "Delete vehicle", "Delete", "Removed", "", "", ""],
        ["History", "List history", "Open history", "Entries shown", "", "", ""],
        ["History", "Re-analyze", "Context menu", "Re-analysis runs", "", "", ""],
        ["Accuracy", "Stats display", "Open accuracy", "Stats visible", "", "", ""],
        ["Settings", "Backend change", "Select backend", "Value saved", "", "", ""],
        ["Settings", "Usage mode", "Select mode", "Value saved", "", "", ""],
        ["Settings", "Model change", "Edit model", "Value saved", "", "", ""],
        ["Import", "Legacy import", "Load JSON", "Preview ok", "", "", ""],
    ];

    for (idx, row) in rows.iter().enumerate() {
        let r = (idx + 1) as u32;
        for (col, value) in row.iter().enumerate() {
            sheet.write_string(r, col as u16, *value)?;
        }
    }

    sheet.set_column_width(0, 14)?;
    sheet.set_column_width(1, 22)?;
    sheet.set_column_width(2, 28)?;
    sheet.set_column_width(3, 22)?;
    sheet.set_column_width(4, 22)?;
    sheet.set_column_width(5, 10)?;
    sheet.set_column_width(6, 30)?;

    workbook.save(output)?;
    Ok(())
}
