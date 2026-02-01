//! Output formatting module

use crate::cli::OutputFormat;
use crate::constants::get_truck_spec;
use crate::error::Result;
use crate::types::{EstimationResult, LoadGrade};

pub fn output_result(output_format: OutputFormat, result: &EstimationResult) -> Result<()> {
    if output_format == OutputFormat::Json {
        let content = serde_json::to_string_pretty(result)?;
        println!("{}", content);
    } else {
        // Table format
        println!("\nAnalysis Result");
        println!("===============");
        println!(
            "Target detected: {}",
            if result.is_target_detected {
                "Yes"
            } else {
                "No"
            }
        );

        if result.is_target_detected {
            println!("Truck type:      {}", result.truck_type);
            println!("Material:        {}", result.material_type);
            println!("Volume:          {:.2} m³", result.estimated_volume_m3);
            println!("Tonnage:         {:.2} t", result.estimated_tonnage);

            if let Some(max_cap) = result.estimated_max_capacity {
                let load_pct = (result.estimated_tonnage / max_cap) * 100.0;
                let grade = LoadGrade::from_ratio(result.estimated_tonnage / max_cap);
                println!("Max capacity:    {:.1} t", max_cap);
                println!("Load:            {:.1}% ({})", load_pct, grade.label());
            } else if let Some(spec) = get_truck_spec(&result.truck_type) {
                let load_pct = (result.estimated_tonnage / spec.max_capacity) * 100.0;
                let grade = LoadGrade::from_ratio(result.estimated_tonnage / spec.max_capacity);
                println!("Max capacity:    {:.1} t (standard)", spec.max_capacity);
                println!("Load:            {:.1}% ({})", load_pct, grade.label());
            }

            println!("Confidence:      {:.0}%", result.confidence_score * 100.0);

            if let Some(ref plate) = result.license_plate {
                println!("License plate:   {}", plate);
            }

            println!("\nReasoning:");
            println!("{}", result.reasoning);
        }
    }

    Ok(())
}
