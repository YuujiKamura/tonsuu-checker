//! Output formatting module

use tonsuu_types::OutputFormat;
use tonsuu_types::Result;
use tonsuu_types::{EstimationResult, LoadGrade};

pub fn output_result(output_format: OutputFormat, result: &EstimationResult, max_capacity: Option<f64>) -> Result<()> {
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

            // Show intermediate calculation values if available
            println!("\n--- Volume Estimation ---");
            if let Some(h) = result.height {
                println!("Height:          {:.2} m", h);
            }
            if let Some(l) = result.fill_ratio_l {
                println!("Fill ratio L:    {:.2}", l);
            }
            println!("-------------------------");

            println!("Volume:          {:.2} m³", result.estimated_volume_m3);
            println!("Tonnage:         {:.2} t", result.estimated_tonnage);

            // Show load ratio if max capacity is known
            if let Some(cap) = max_capacity {
                let load_pct = (result.estimated_tonnage / cap) * 100.0;
                let grade = LoadGrade::from_ratio(result.estimated_tonnage / cap);
                println!("Max capacity:    {:.1} t", cap);
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
