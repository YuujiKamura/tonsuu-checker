//! Ground truth regression test
//!
//! tests/fixtures/ground_truth.json の画像を解析し、
//! AIの判断した中間値（高さ・充填率等）と最終推定値を記録する。
//! 結果は tests/fixtures/last_run.json に保存される。
//!
//! 使い方 (環境変数):
//!   $env:TONSUU_GT_INDEX="1"; cargo test -p tonsuu-cli --test ground_truth_test -- --nocapture
//!   $env:TONSUU_GT_NUMBER="1122"; cargo test -p tonsuu-cli --test ground_truth_test -- --nocapture
//!   $env:TONSUU_GT_RANK="low"; cargo test -p tonsuu-cli --test ground_truth_test -- --nocapture
//!   $env:TONSUU_GT_ALL="1"; cargo test -p tonsuu-cli --test ground_truth_test -- --nocapture
//!   $env:TONSUU_GT_INDEX="1"; $env:TONSUU_GT_HEIGHT_ONLY="1"; cargo test -p tonsuu-cli --test ground_truth_test -- --nocapture

use serde::{Deserialize, Serialize};
use std::env;
use std::path::PathBuf;
use tonsuu_vision::{analyze_image, AnalyzerConfig};

fn fixtures_dir() -> PathBuf {
    PathBuf::from(env!("CARGO_MANIFEST_DIR"))
        .join("..")
        .join("..")
        .join("tests")
        .join("fixtures")
}

#[derive(Debug, Deserialize, Clone)]
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
    height: Option<f64>,
    packing_density: Option<f64>,
    fill_ratio_l: Option<f64>,
    fill_ratio_w: Option<f64>,
    fill_ratio_z: Option<f64>,
    // AI final values
    estimated_volume_m3: f64,
    estimated_tonnage: f64,
    confidence_score: f64,
    reasoning: String,
    // Comparison
    error: f64,
    error_pct: f64,
}

#[derive(Debug, Serialize)]
struct HeightRunResult {
    file: String,
    description: String,
    height: Option<f64>,
    confidence_score: f64,
    reasoning: String,
}

fn default_config() -> AnalyzerConfig {
    AnalyzerConfig::default().with_model(Some("gemini-3-flash-preview".to_string()))
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum TonnageRank {
    Low,
    Mid,
    High,
}

impl TonnageRank {
    fn from_tonnage(value: f64) -> Self {
        if value <= 3.2 {
            TonnageRank::Low
        } else if value < 4.0 {
            TonnageRank::Mid
        } else {
            TonnageRank::High
        }
    }

    fn parse(s: &str) -> Option<Self> {
        match s.to_lowercase().as_str() {
            "low" => Some(TonnageRank::Low),
            "mid" | "middle" => Some(TonnageRank::Mid),
            "high" => Some(TonnageRank::High),
            _ => None,
        }
    }
}

#[derive(Debug, Default)]
struct SelectionArgs {
    index: Option<usize>,
    number: Option<String>,
    rank: Option<TonnageRank>,
    all: bool,
    height_only: bool,
}

fn env_bool(key: &str) -> bool {
    match env::var(key).ok().as_deref() {
        Some("1") | Some("true") | Some("TRUE") | Some("yes") | Some("YES") => true,
        _ => false,
    }
}

fn parse_args() -> SelectionArgs {
    let mut args = SelectionArgs::default();
    args.index = env::var("TONSUU_GT_INDEX").ok().and_then(|v| v.parse::<usize>().ok());
    args.number = env::var("TONSUU_GT_NUMBER").ok();
    args.rank = env::var("TONSUU_GT_RANK").ok().and_then(|v| TonnageRank::parse(&v));
    args.all = env_bool("TONSUU_GT_ALL");
    args.height_only = env_bool("TONSUU_GT_HEIGHT_ONLY");
    args
}

fn entry_matches_number(entry: &GroundTruthEntry, number: &str) -> bool {
    let want: String = number.chars().filter(|c| c.is_ascii_digit()).collect();
    if want.is_empty() {
        return false;
    }
    let haystack = format!("{} {}", entry.file, entry.description);
    let digits: String = haystack.chars().filter(|c| c.is_ascii_digit()).collect();
    digits.contains(&want)
}

fn load_ground_truth() -> Vec<GroundTruthEntry> {
    let path = fixtures_dir().join("ground_truth.json");
    let content =
        std::fs::read_to_string(&path).unwrap_or_else(|e| panic!("ground_truth.json not found: {}", e));
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
        height: result.height,
        packing_density: result.packing_density,
        fill_ratio_l: result.fill_ratio_l,
        fill_ratio_w: result.fill_ratio_w,
        fill_ratio_z: result.fill_ratio_z,
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
    println!("    height:        {:?} m", r.height);
    println!("    packing:       {:?}", r.packing_density);
    println!("    fill_ratio_l:  {:?}", r.fill_ratio_l);
    println!("    fill_ratio_w:  {:?}", r.fill_ratio_w);
    println!("    fill_ratio_z:  {:?}", r.fill_ratio_z);
    println!("    volume:        {:.2} m³", r.estimated_volume_m3);
    println!("    confidence:    {:.0}%", r.confidence_score * 100.0);
    println!("  結果:");
    println!(
        "    推定: {:.2} t  /  実測: {:.2} t  /  誤差: {:+.2} t ({:+.1}%)",
        r.estimated_tonnage, r.actual_tonnage, r.error, r.error_pct
    );
    println!("  reasoning: {}", r.reasoning);
}

fn save_results(results: &[RunResult]) {
    let path = fixtures_dir().join("last_run.json");
    let content = serde_json::to_string_pretty(results).expect("Failed to serialize");
    std::fs::write(&path, content).expect("Failed to write last_run.json");
    println!("\n結果を保存: {}", path.display());
}

fn save_height_results(results: &[HeightRunResult]) {
    let path = fixtures_dir().join("height_last_run.json");
    let content = serde_json::to_string_pretty(results).expect("Failed to serialize");
    std::fs::write(&path, content).expect("Failed to write height_last_run.json");
    println!("\n結果を保存: {}", path.display());
}

#[test]
fn ground_truth_selected() {
    let args = parse_args();
    let entries = load_ground_truth();
    let config = default_config();

    if entries.is_empty() {
        panic!("ground_truth.json is empty");
    }

    let selection = if let Some(index) = args.index {
        if index == 0 || index > entries.len() {
            panic!("index out of range: {} (1..={})", index, entries.len());
        }
        vec![entries[index - 1].clone()]
    } else if let Some(ref number) = args.number {
        let matched: Vec<_> = entries
            .iter()
            .filter(|e| entry_matches_number(e, number))
            .cloned()
            .collect();
        if matched.is_empty() {
            panic!("no entry matched number: {}", number);
        }
        matched
    } else if let Some(rank) = args.rank {
        let matched: Vec<_> = entries
            .iter()
            .filter(|e| TonnageRank::from_tonnage(e.actual_tonnage) == rank)
            .cloned()
            .collect();
        if matched.is_empty() {
            panic!("no entry matched rank");
        }
        matched
    } else if args.all {
        entries.clone()
    } else {
        println!("No selection specified. Use TONSUU_GT_INDEX/NUMBER/RANK/ALL.");
        return;
    };

    if args.height_only {
        let mut results = Vec::new();
        println!("\n=== Height Only Test ({} images) ===\n", selection.len());
        for entry in &selection {
            let image_path = fixtures_dir().join(&entry.file);
            assert!(image_path.exists(), "Image not found: {}", image_path.display());

            let result = analyze_image(&image_path, &config)
                .unwrap_or_else(|e| panic!("Analysis failed for {}: {}", entry.file, e));

            let r = HeightRunResult {
                file: entry.file.clone(),
                description: entry.description.clone(),
                height: result.height,
                confidence_score: result.confidence_score,
                reasoning: result.reasoning,
            };

            println!("─────────────────────────────────────");
            println!("  {}", r.description);
            println!("  file: {}", r.file);
            println!("    height:      {:?} m", r.height);
            println!("    confidence:  {:.0}%", r.confidence_score * 100.0);
            println!("    reasoning:   {}", r.reasoning);

            results.push(r);
        }
        save_height_results(&results);
        return;
    }

    let mut results = Vec::new();

    println!("\n=== Ground Truth Test ({} images) ===\n", selection.len());

    for entry in &selection {
        let r = run_single(entry, &config);
        print_result(&r);
        results.push(r);
    }

    if selection.len() > 1 {
        println!("\n═══════════════════════════════════════");
        println!("  Summary ({} images)", results.len());
        println!("═══════════════════════════════════════");

        let mean_abs_error: f64 = results.iter().map(|r| r.error.abs()).sum::<f64>() / results.len() as f64;
        let mean_error: f64 = results.iter().map(|r| r.error).sum::<f64>() / results.len() as f64;
        let rmse: f64 =
            (results.iter().map(|r| r.error * r.error).sum::<f64>() / results.len() as f64).sqrt();

        for r in &results {
            println!(
                "  {:<35} est {:.2}t  act {:.2}t  err {:+.2}t",
                r.file, r.estimated_tonnage, r.actual_tonnage, r.error
            );
        }
        println!("  ---");
        println!("  Mean Error:     {:+.3} t", mean_error);
        println!("  Mean Abs Error: {:.3} t", mean_abs_error);
        println!("  RMSE:           {:.3} t", rmse);
    }

    save_results(&results);
}
