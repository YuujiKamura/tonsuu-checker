//! Ground truth regression test
//!
//! fixtures/ground_truth.json の画像を解析し、
//! AIの判断した中間値（上面積・高さ・空隙率等）と最終推定値を記録する。
//! 結果は tests/fixtures/last_run.json に保存される。
//!
//! 実行: cargo test --test ground_truth_test -- --ignored --nocapture
//! 特定画像のみ: cargo test --test ground_truth_test red_1122_heaped -- --ignored --nocapture

use serde::{Deserialize, Serialize};
use std::path::PathBuf;
use tonsuu_checker::analyzer::{analyze_image, AnalyzerConfig};

fn fixtures_dir() -> PathBuf {
    PathBuf::from(env!("CARGO_MANIFEST_DIR"))
        .join("tests")
        .join("fixtures")
}

#[derive(Debug, Deserialize)]
struct GroundTruthEntry {
    file: String,
    description: String,
    actual_tonnage: f64,
    #[allow(dead_code)]
    truck_class: String,
    #[allow(dead_code)]
    material: String,
}

#[derive(Debug, Serialize)]
struct RunResult {
    file: String,
    description: String,
    actual_tonnage: f64,
    // AI intermediate values
    truck_type: String,
    material_type: String,
    upper_area: Option<f64>,
    height: Option<f64>,
    slope: Option<f64>,
    void_ratio: Option<f64>,
    // AI final values
    estimated_volume_m3: f64,
    estimated_tonnage: f64,
    confidence_score: f64,
    reasoning: String,
    // Comparison
    error: f64,
    error_pct: f64,
}

fn default_config() -> AnalyzerConfig {
    AnalyzerConfig::default()
        .with_model(Some("gemini-2.5-pro".to_string()))
}

fn load_ground_truth() -> Vec<GroundTruthEntry> {
    let path = fixtures_dir().join("ground_truth.json");
    let content = std::fs::read_to_string(&path)
        .unwrap_or_else(|e| panic!("ground_truth.json not found: {}", e));
    serde_json::from_str(&content).expect("Failed to parse ground_truth.json")
}

fn run_single(entry: &GroundTruthEntry, config: &AnalyzerConfig) -> RunResult {
    let image_path = fixtures_dir().join(&entry.file);
    assert!(image_path.exists(), "Image not found: {}", image_path.display());

    let result = analyze_image(&image_path, config)
        .unwrap_or_else(|e| panic!("Analysis failed for {}: {}", entry.file, e));

    let error = result.estimated_tonnage - entry.actual_tonnage;
    let error_pct = if entry.actual_tonnage > 0.0 {
        (error / entry.actual_tonnage) * 100.0
    } else {
        0.0
    };

    RunResult {
        file: entry.file.clone(),
        description: entry.description.clone(),
        actual_tonnage: entry.actual_tonnage,
        truck_type: result.truck_type,
        material_type: result.material_type,
        upper_area: result.upper_area,
        height: result.height,
        slope: result.slope,
        void_ratio: result.void_ratio,
        estimated_volume_m3: result.estimated_volume_m3,
        estimated_tonnage: result.estimated_tonnage,
        confidence_score: result.confidence_score,
        reasoning: result.reasoning,
        error,
        error_pct,
    }
}

fn print_result(r: &RunResult) {
    println!("─────────────────────────────────────");
    println!("  {}", r.description);
    println!("  file: {}", r.file);
    println!("  AI判断:");
    println!("    truck_type:    {}", r.truck_type);
    println!("    material_type: {}", r.material_type);
    println!("    upper_area:    {:?} m²", r.upper_area);
    println!("    height:        {:?} m", r.height);
    println!("    slope:         {:?}°", r.slope);
    println!("    void_ratio:    {:?}", r.void_ratio);
    println!("    volume:        {:.2} m³", r.estimated_volume_m3);
    println!("    confidence:    {:.0}%", r.confidence_score * 100.0);
    println!("  結果:");
    println!("    推定: {:.2} t  /  実測: {:.2} t  /  誤差: {:+.2} t ({:+.1}%)",
        r.estimated_tonnage, r.actual_tonnage, r.error, r.error_pct);
    println!("  reasoning: {}", r.reasoning);
}

fn save_results(results: &[RunResult]) {
    let path = fixtures_dir().join("last_run.json");
    let content = serde_json::to_string_pretty(results).expect("Failed to serialize");
    std::fs::write(&path, content).expect("Failed to write last_run.json");
    println!("\n結果を保存: {}", path.display());
}

/// 全画像を解析して中間値を記録
#[test]
#[ignore]
fn ground_truth_all() {
    let entries = load_ground_truth();
    let config = default_config();

    println!("\n=== Ground Truth Test ({} images) ===\n", entries.len());

    let mut handles = Vec::new();
    for (idx, entry) in entries.into_iter().enumerate() {
        let cfg = config.clone();
        let handle = std::thread::spawn(move || {
            let result = run_single(&entry, &cfg);
            (idx, result)
        });
        handles.push(handle);
    }

    let mut results = Vec::new();
    for handle in handles {
        match handle.join() {
            Ok((idx, result)) => results.push((idx, result)),
            Err(_) => panic!("Ground truth thread panicked"),
        }
    }

    results.sort_by_key(|(idx, _)| *idx);
    let results: Vec<RunResult> = results.into_iter().map(|(_, r)| r).collect();

    for r in &results {
        print_result(r);
    }

    // Summary
    println!("\n═══════════════════════════════════════");
    println!("  Summary ({} images)", results.len());
    println!("═══════════════════════════════════════");

    let mean_abs_error: f64 = results.iter().map(|r| r.error.abs()).sum::<f64>() / results.len() as f64;
    let mean_error: f64 = results.iter().map(|r| r.error).sum::<f64>() / results.len() as f64;
    let rmse: f64 = (results.iter().map(|r| r.error * r.error).sum::<f64>() / results.len() as f64).sqrt();

    for r in &results {
        println!("  {:<35} est {:.2}t  act {:.2}t  err {:+.2}t",
            r.file, r.estimated_tonnage, r.actual_tonnage, r.error);
    }
    println!("  ---");
    println!("  Mean Error:     {:+.3} t", mean_error);
    println!("  Mean Abs Error: {:.3} t", mean_abs_error);
    println!("  RMSE:           {:.3} t", rmse);

    save_results(&results);
}

// --- Individual tests (run one image at a time) ---

macro_rules! ground_truth_single {
    ($name:ident, $file:expr) => {
        #[test]
        #[ignore]
        fn $name() {
            let entries = load_ground_truth();
            let entry = entries.iter().find(|e| e.file == $file)
                .unwrap_or_else(|| panic!("Entry not found: {}", $file));
            let config = default_config();
            let r = run_single(entry, &config);
            print_result(&r);
            save_results(&[r]);
        }
    };
}

ground_truth_single!(gt_isuzu_white_1121_disposal, "isuzu_white_1121_disposal.jpg");
ground_truth_single!(gt_white_1177_loading, "white_1177_loading.jpg");
ground_truth_single!(gt_white_1177_side, "white_1177_side.jpg");
ground_truth_single!(gt_red_1122_heaped, "red_1122_heaped.jpg");
ground_truth_single!(gt_isuzu_green_8267_disposal, "isuzu_green_8267_disposal.jpg");
ground_truth_single!(gt_red_1122_disposal, "red_1122_disposal.jpg");
ground_truth_single!(gt_isuzu_white_cloudy_heaped, "isuzu_white_cloudy_heaped.jpg");
