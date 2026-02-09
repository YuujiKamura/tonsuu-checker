//! Command handlers

use tonsuu_vision::cache::Cache;
use tonsuu_vision::AnalyzerConfig;
use tonsuu_app::app::{self, AnalysisOptions};
use cli_ai_analyzer::check_gemini_status;
use crate::cli::{Cli, Commands, OutputFormat};
use tonsuu_app::config::Config;
use tonsuu_app::repository::{open_history_store, open_vehicle_store};
use tonsuu_app::constants::get_truck_spec;
use tonsuu_types::{Error, Result};
use tonsuu_app::export::export_to_excel;
use crate::output::output_result;
use tonsuu_app::scanner::{scan_directory, validate_image};
use tonsuu_store::{HistoryEntry, VehicleStore};
use tonsuu_domain::service::{check_overloads, generate_overload_report};
use tonsuu_infra::overload_csv::{load_slips_from_csv, load_vehicles_from_csv};
use tonsuu_types::{AnalysisEntry, BatchResults, EstimationResult, KarteInput, LoadGrade, RegisteredVehicle, TruckClass};
use chrono::Utc;
use indicatif::{MultiProgress, ProgressBar, ProgressStyle};
use serde::Deserialize;
use std::path::PathBuf;
use std::sync::atomic::{AtomicUsize, Ordering};
use std::sync::{Arc, Mutex};
use std::thread;
use std::time::Instant;

/// Performance profiler for analysis
#[derive(Debug, Default)]
struct AnalysisProfiler {
    total_start: Option<Instant>,
    yolo_ms: Option<u64>,
    api_ms: Option<u64>,
    stage2_ms: Option<u64>,
    cache_hit: bool,
}

impl AnalysisProfiler {
    fn new() -> Self {
        Self {
            total_start: Some(Instant::now()),
            ..Default::default()
        }
    }

    #[allow(dead_code)]
    fn record_yolo(&mut self, start: Instant) {
        self.yolo_ms = Some(start.elapsed().as_millis() as u64);
    }

    #[allow(dead_code)]
    fn record_api(&mut self, start: Instant) {
        self.api_ms = Some(start.elapsed().as_millis() as u64);
    }

    fn record_stage2(&mut self, start: Instant) {
        self.stage2_ms = Some(start.elapsed().as_millis() as u64);
    }

    fn print_summary(&self) {
        let total_ms = self.total_start.map(|s| s.elapsed().as_millis() as u64).unwrap_or(0);

        eprintln!("\n⏱ Profile:");
        if self.cache_hit {
            eprintln!("  Cache hit - {:.1}s total", total_ms as f64 / 1000.0);
            return;
        }

        let mut breakdown = Vec::new();
        if let Some(ms) = self.yolo_ms {
            breakdown.push(format!("YOLO {:.1}s", ms as f64 / 1000.0));
        }
        if let Some(ms) = self.api_ms {
            breakdown.push(format!("API {:.1}s", ms as f64 / 1000.0));
        }
        if let Some(ms) = self.stage2_ms {
            breakdown.push(format!("Stage2 {:.1}s", ms as f64 / 1000.0));
        }

        if breakdown.is_empty() {
            eprintln!("  Total: {:.1}s", total_ms as f64 / 1000.0);
        } else {
            eprintln!("  {} | Total: {:.1}s", breakdown.join(" + "), total_ms as f64 / 1000.0);
        }
    }
}

/// Result from Gemini plate OCR
#[allow(dead_code)]
#[derive(Debug, Deserialize)]
struct PlateOcrResult {
    plate: Option<String>,
    confidence: Option<f32>,
}

/// Build a simple OCR prompt for cropped plate image
#[allow(dead_code)]
fn build_plate_ocr_prompt(vehicle_store: &VehicleStore) -> String {
    let mut prompt = String::from(
r#"この画像は日本の自動車ナンバープレートです。プレートに書かれている文字を正確に読み取ってください。

【読み取り手順】
1. 地名（例: 熊本、福岡、東京）
2. 分類番号3桁（例: 130, 101, 500）
3. ひらがな1文字（例: ら, あ, さ）
4. 一連番号4桁（例: 1122, 5678）← ハイフンがある場合は除去して4桁で

【重要】
- 見えた文字のみを記載すること
- 推測・創作は禁止
- 読み取れない部分は「?」で表記

"#);

    // Add registered vehicles for matching hint
    let vehicles: Vec<_> = vehicle_store.all_vehicles();
    if !vehicles.is_empty() {
        prompt.push_str("【登録車両リスト（参考）】以下のナンバーが登録されています:\n");
        for v in vehicles {
            if let Some(ref plate) = v.license_plate {
                prompt.push_str(&format!("- {}\n", plate));
            }
        }
        prompt.push_str("\n読み取った結果がリストにあればそのまま返す。なければ読み取った通りに返す。\n\n");
    }

    prompt.push_str(r#"以下のJSON形式で回答:
{"plate": "読み取ったナンバー全体", "confidence": 0.0-1.0}

読み取れない場合: {"plate": null, "confidence": 0.0}"#);

    prompt
}

/// Execute CLI command
pub fn execute(cli: Cli) -> Result<()> {
    // Load config
    let mut config = Config::load()?;

    // Override from CLI args
    if let Some(ref backend) = cli.backend {
        config.backend = backend.clone();
    }
    if cli.model.is_some() {
        config.model = cli.model.clone();
    }
    if let Some(ref usage_mode) = cli.usage_mode {
        config.usage_mode = usage_mode.clone();
    }

    match &cli.command {
        Commands::Analyze {
            image,
            no_cache,
            ensemble,
            plate,
            skip_yolo_class_only,
            company,
            karte,
            material,
            truck_class,
        } => {
            // Use CLI ensemble if specified, otherwise config value
            let ensemble_count = ensemble.unwrap_or(config.ensemble_count);
            // Cache disabled if: --no-cache OR config.cache_enabled=false
            let use_cache = !no_cache && config.cache_enabled;
            let output_format = cli.format.unwrap_or(config.output_format);
            cmd_analyze(
                &cli,
                &config,
                image.clone(),
                use_cache,
                ensemble_count,
                output_format,
                plate.clone(),
                skip_yolo_class_only.clone(),
                company.clone(),
                karte.clone(),
                material.clone(),
                truck_class.clone(),
            )
        }

        Commands::Batch {
            folder,
            output,
            no_cache,
            jobs,
        } => {
            // Use CLI jobs if specified, otherwise default 4. 0 = auto CPU count.
            let job_count = match jobs {
                Some(0) => num_cpus::get(),
                Some(n) => *n,
                None => 4,
            };
            // Cache disabled if: --no-cache OR config.cache_enabled=false
            let use_cache = !no_cache && config.cache_enabled;
            let output_format = cli.format.unwrap_or(config.output_format);
            cmd_batch(&cli, &config, folder.clone(), output.clone(), use_cache, job_count, output_format)
        }

        Commands::Export { results, output } => cmd_export(results.clone(), output.clone()),

        Commands::Config {
            show,
            set_backend,
            set_model,
            set_cache,
            set_output,
            set_ensemble,
            set_plate_local,
            set_plate_local_cmd,
            set_plate_local_min_conf,
            set_plate_local_fallback,
            set_usage_mode,
            reset,
        } => cmd_config(
            *show,
            set_backend.clone(),
            set_model.clone(),
            *set_cache,
            *set_output,
            *set_ensemble,
            *set_plate_local,
            set_plate_local_cmd.clone(),
            *set_plate_local_min_conf,
            *set_plate_local_fallback,
            set_usage_mode.clone(),
            *reset,
        ),

        Commands::Cache { clear, stats } => cmd_cache(&config, *clear, *stats),

        Commands::Feedback {
            image,
            actual,
            notes,
        } => cmd_feedback(&config, image.clone(), *actual, notes.clone()),

        Commands::History {
            with_feedback,
            limit,
        } => cmd_history(&config, *with_feedback, *limit),

        Commands::Accuracy {
            by_truck,
            by_material,
            detailed,
        } => cmd_accuracy(&config, *by_truck, *by_material, *detailed),

        Commands::AutoCollect {
            folder,
            yes,
            jobs,
            dry_run,
            company,
        } => cmd_auto_collect(&cli, &config, folder.clone(), *yes, *jobs, *dry_run, company.clone()),

        Commands::Import { file, dry_run } => cmd_import(&config, file.clone(), *dry_run),

        Commands::Stats => cmd_stats(&cli),

        Commands::CheckOverload {
            csv,
            vehicles,
            output,
        } => cmd_check_overload(csv.clone(), vehicles.clone(), output.unwrap_or(OutputFormat::Table)),
    }
}

fn cmd_analyze(
    cli: &Cli,
    config: &Config,
    image: PathBuf,
    use_cache: bool,
    ensemble: u32,
    output_format: OutputFormat,
    manual_plate: Option<String>,
    skip_yolo_class_only: Option<String>,
    filter_company: Option<String>,
    karte_arg: Option<String>,
    material_type: Option<String>,
    truck_type_hint: Option<String>,
) -> Result<()> {
    // Initialize profiler
    let mut profiler = AnalysisProfiler::new();

    // Parse skip_yolo_class_only to get TruckClass
    let truck_class_override: Option<TruckClass> =
        if let Some(ref class_name) = skip_yolo_class_only {
            let truck_class = match class_name.as_str() {
                "2t" => TruckClass::TwoTon,
                "4t" => TruckClass::FourTon,
                "増トン" => TruckClass::IncreasedTon,
                "10t" => TruckClass::TenTon,
                _ => {
                    eprintln!("警告: 不明なクラス名 '{}' (2t, 4t, 増トン, 10t のいずれかを指定)", class_name);
                    TruckClass::Unknown
                }
            };
            Some(truck_class)
        } else {
            None
        };

    // Build analysis options using the app layer
    let karte_json = match karte_arg {
        Some(arg) => Some(parse_karte_arg(&arg)?),
        None => None,
    };

    let mut options = AnalysisOptions::new()
        .with_cache(use_cache)
        .with_ensemble_count(ensemble)
        .with_verbose(cli.verbose);

    if let Some(karte) = karte_json {
        options = options.with_karte_json(karte);
    }

    if let Some(plate) = manual_plate {
        options = options.with_manual_plate(plate);
    }

    if let Some(class) = truck_class_override {
        options = options.with_truck_class(class);
    }

    if let Some(company) = filter_company {
        options = options.with_company_filter(company);
    }

    if let Some(material) = material_type {
        options = options.with_material_type(material);
    }

    if let Some(truck_type) = truck_type_hint {
        options = options.with_truck_type_hint(truck_type);
    }

    // Create progress callback for verbose mode
    let progress_cb = if cli.verbose {
        Some(Box::new(|msg: &str| eprintln!("  {}", msg)) as tonsuu_vision::ProgressCallback)
    } else {
        None
    };

    if cli.verbose {
        eprintln!("Analyzing image: {}", image.display());
    }

    // Delegate to app layer
    let analysis_start = Instant::now();
    let result = app::analyze_truck_image(&image, config, &options, progress_cb)
        .map_err(|e: app::AnalysisServiceError| Error::AnalysisFailed(e.to_string()))?;
    profiler.record_stage2(analysis_start);

    if result.from_cache {
        profiler.cache_hit = true;
        if cli.verbose {
            eprintln!("Using cached result");
        }
    }

    // Output vehicle info if matched
    if let Some(ref vehicle) = result.matched_vehicle {
        if cli.verbose {
            eprintln!(
                "登録車両と照合: {} ({}t) - {}",
                vehicle.name,
                vehicle.max_capacity,
                vehicle.license_plate.as_deref().unwrap_or("N/A")
            );
        }
        println!("\n=== 登録車両情報 ===");
        println!("車両名:     {}", vehicle.name);
        println!("最大積載量: {}t", vehicle.max_capacity);
        println!("ナンバー:   {}", vehicle.license_plate.as_deref().unwrap_or("-"));
        println!("クラス:     {}", vehicle.truck_class().label());
    } else if cli.verbose {
        if let Some(ref class_name) = skip_yolo_class_only {
            let max_cap = match class_name.as_str() {
                "2t" => 2.0,
                "4t" => 4.0,
                "増トン" => 6.5,
                "10t" => 10.0,
                _ => 0.0,
            };
            eprintln!("クラス指定: {} (参照用積載量: {}t、YOLO車両特定スキップ、積載率計算なし)",
                class_name, max_cap);
        } else {
            eprintln!("登録車両との照合: 該当なし");
        }
    }

    // Output result
    // For skip_yolo_class_only mode, don't pass max_capacity (no load ratio calculation)
    // For matched vehicle, pass vehicle's max_capacity
    let output_capacity = result.matched_vehicle.as_ref().map(|v| v.max_capacity);
    output_result(output_format, &result.estimation, output_capacity)?;
    profiler.print_summary();

    Ok(())
}

fn parse_karte_arg(arg: &str) -> Result<String> {
    let path = PathBuf::from(arg);
    let raw = if path.exists() {
        std::fs::read_to_string(&path).map_err(Error::Io)?
    } else {
        arg.to_string()
    };

    let _karte: KarteInput = serde_json::from_str(&raw).map_err(Error::Json)?;
    // Normalize JSON to ensure valid formatting
    serde_json::to_string(&_karte).map_err(Error::Json)
}

/// Result from a single analysis task
#[derive(Debug)]
struct AnalysisTaskResult {
    image_path: PathBuf,
    result: std::result::Result<EstimationResult, String>,
}

fn cmd_batch(
    cli: &Cli,
    config: &Config,
    folder: PathBuf,
    output: Option<PathBuf>,
    use_cache: bool,
    jobs: usize,
    output_format: OutputFormat,
) -> Result<()> {
    // Scan directory
    let images = scan_directory(&folder)?;

    if images.is_empty() {
        return Err(Error::FileNotFound(format!(
            "No images found in {}",
            folder.display()
        )));
    }

    let total_images = images.len();
    if cli.verbose {
        eprintln!(
            "Found {} images to analyze with {} parallel jobs (cache: {})",
            total_images, jobs, if use_cache { "on" } else { "off" }
        );
    }

    // Setup progress bar
    let multi_progress = MultiProgress::new();
    let main_pb = multi_progress.add(ProgressBar::new(total_images as u64));
    main_pb.set_style(
        ProgressStyle::default_bar()
            .template("{spinner:.green} [{elapsed_precise}] [{bar:40.cyan/blue}] {pos}/{len} ({eta}) {msg}")
            .unwrap()
            .progress_chars("#>-"),
    );

    // Shared results collector
    let results: Arc<Mutex<Vec<AnalysisTaskResult>>> = Arc::new(Mutex::new(Vec::new()));
    let images = Arc::new(images);
    let next_index = Arc::new(AtomicUsize::new(0));

    // Track timing
    let started_at = Utc::now();

    // Spawn worker threads
    let mut handles = Vec::new();
    let verbose = cli.verbose;

    for worker_id in 0..jobs {
        let images = Arc::clone(&images);
        let next_index = Arc::clone(&next_index);
        let results = Arc::clone(&results);
        let config = config.clone();
        let pb = main_pb.clone();

        let handle = thread::spawn(move || {
            let batch_options = AnalysisOptions::new()
                .with_cache(use_cache)
                .with_ensemble_count(config.ensemble_count);

            loop {
                // Get next image to process (lock-free)
                let idx = next_index.fetch_add(1, Ordering::SeqCst);
                if idx >= images.len() {
                    break;
                }

                let image = &images[idx];

                // Update progress message
                let filename = image
                    .file_name()
                    .and_then(|n| n.to_str())
                    .unwrap_or("")
                    .to_string();

                if verbose {
                    pb.set_message(format!("[W{}] {}", worker_id, filename));
                }

                // Use app layer (box-overlay pipeline by default)
                let result = app::analyze_truck_image(image, &config, &batch_options, None)
                    .map(|r| r.estimation)
                    .map_err(|e| e.to_string());

                // Store result
                {
                    let mut results_guard = results.lock().unwrap();
                    results_guard.push(AnalysisTaskResult {
                        image_path: image.clone(),
                        result,
                    });
                }

                pb.inc(1);
            }
        });

        handles.push(handle);
    }

    // Wait for all workers to complete
    for handle in handles {
        let _ = handle.join();
    }

    main_pb.finish_with_message("Complete");

    let completed_at = Utc::now();

    // Collect results
    let task_results = Arc::try_unwrap(results)
        .expect("All workers should be done")
        .into_inner()
        .unwrap();

    // Convert to entries
    let mut entries = Vec::new();
    let mut successful = 0;
    let mut failed = 0;

    for task_result in task_results {
        match task_result.result {
            Ok(result) => {
                // Calculate grade from truck spec
                let grade = if let Some(spec) = get_truck_spec(&result.truck_type) {
                    Some(LoadGrade::from_ratio(
                        result.estimated_tonnage / spec.max_capacity,
                    ))
                } else {
                    None
                };

                entries.push(AnalysisEntry {
                    image_path: task_result.image_path.display().to_string(),
                    timestamp: Utc::now(),
                    result,
                    grade,
                    actual_tonnage: None,
                });
                successful += 1;
            }
            Err(e) => {
                if cli.verbose {
                    eprintln!("Failed to analyze {}: {}", task_result.image_path.display(), e);
                }
                failed += 1;
            }
        }
    }

    // Sort entries by image path for consistent output
    entries.sort_by(|a, b| a.image_path.cmp(&b.image_path));

    // Save to history store
    if let Ok(mut store) = open_history_store(config) {
        for entry in &entries {
            let path = std::path::Path::new(&entry.image_path);
            let _ = store.add_analysis(path, entry.result.clone());
        }
    }

    let results = BatchResults {
        entries,
        total_processed: total_images,
        successful,
        failed,
        started_at,
        completed_at,
    };

    // Output results
    if let Some(output_path) = output {
        let content = serde_json::to_string_pretty(&results)?;
        std::fs::write(&output_path, content)?;
        println!("Results saved to: {}", output_path.display());
    } else {
        // Print summary
        println!("\nBatch Analysis Complete");
        println!("=======================");
        println!("Total:      {}", results.total_processed);
        println!("Successful: {}", results.successful);
        println!("Failed:     {}", results.failed);
        println!(
            "Duration:   {:.1}s",
            (results.completed_at - results.started_at).num_milliseconds() as f64 / 1000.0
        );

        if output_format == OutputFormat::Json {
            let content = serde_json::to_string_pretty(&results)?;
            println!("\n{}", content);
        }
    }

    Ok(())
}

fn cmd_export(results_path: PathBuf, output: Option<PathBuf>) -> Result<()> {
    // Load results
    let content = std::fs::read_to_string(&results_path)?;
    let results: BatchResults = serde_json::from_str(&content)?;

    // Determine output path
    let output_path = output.unwrap_or_else(|| {
        let stem = results_path
            .file_stem()
            .and_then(|s| s.to_str())
            .unwrap_or("results");
        results_path.with_file_name(format!("{}.xlsx", stem))
    });

    // Export to Excel
    export_to_excel(&results, &output_path)?;

    println!("Exported to: {}", output_path.display());
    Ok(())
}

fn cmd_config(
    show: bool,
    set_backend: Option<String>,
    set_model: Option<String>,
    set_cache: Option<bool>,
    set_output: Option<OutputFormat>,
    set_ensemble: Option<u32>,
    set_plate_local: Option<bool>,
    set_plate_local_cmd: Option<String>,
    set_plate_local_min_conf: Option<f32>,
    set_plate_local_fallback: Option<bool>,
    set_usage_mode: Option<String>,
    reset: bool,
) -> Result<()> {
    if reset {
        let config = Config::default();
        config.save()?;
        println!("Configuration reset to defaults");
        println!("\n{}", config);
        return Ok(());
    }

    let mut config = Config::load()?;
    let mut modified = false;

    if let Some(backend) = set_backend {
        config.backend = backend;
        modified = true;
    }

    if let Some(model) = set_model {
        config.model = Some(model);
        modified = true;
    }

    if let Some(cache_enabled) = set_cache {
        config.cache_enabled = cache_enabled;
        modified = true;
    }

    if let Some(output_format) = set_output {
        config.output_format = output_format;
        modified = true;
    }

    if let Some(ensemble_count) = set_ensemble {
        config.ensemble_count = ensemble_count;
        modified = true;
    }

    if let Some(enabled) = set_plate_local {
        config.plate_local_enabled = enabled;
        modified = true;
    }

    if let Some(cmd) = set_plate_local_cmd {
        config.plate_local_command = Some(cmd);
        modified = true;
    }

    if let Some(min_conf) = set_plate_local_min_conf {
        config.plate_local_min_conf = min_conf;
        modified = true;
    }

    if let Some(fallback) = set_plate_local_fallback {
        config.plate_local_fallback_api = fallback;
        modified = true;
    }

    if let Some(usage_mode) = set_usage_mode {
        config.usage_mode = usage_mode;
        modified = true;
    }

    if modified {
        config.save()?;
        println!("Configuration updated");
    }

    if show || !modified {
        println!("{}", config);
    }

    Ok(())
}

fn cmd_cache(config: &Config, clear: bool, stats: bool) -> Result<()> {
    if !config.cache_enabled {
        return Err(Error::Cache(tonsuu_types::CacheError::IoError(
            "Cache is disabled. Enable with: tonsuu-checker config --set-cache true".to_string(),
        )));
    }

    let cache = Cache::new(config.cache_dir()?)?;

    if clear {
        let count = cache.clear()?;
        println!("Cleared {} cached entries", count);
    }

    if stats || !clear {
        let stats = cache.stats()?;
        println!("{}", stats.display());
    }

    Ok(())
}

fn cmd_feedback(
    config: &Config,
    image: PathBuf,
    actual_tonnage: f64,
    notes: Option<String>,
) -> Result<()> {
    validate_image(&image)?;

    let mut store = open_history_store(config)?;

    // Check if entry exists
    if store.get_by_path(&image)?.is_none() {
        return Err(Error::FileNotFound(format!(
            "No analysis found for image: {}. Run 'tonsuu-checker analyze {}' first.",
            image.display(),
            image.display()
        )));
    }

    store.add_feedback(&image, actual_tonnage, notes)?;

    println!("Feedback recorded:");
    println!("  Image:  {}", image.display());
    println!("  Actual: {:.2} t", actual_tonnage);

    // Show comparison with estimate
    if let Some(entry) = store.get_by_path(&image)? {
        let estimated = entry.estimation.estimated_tonnage;
        let error = estimated - actual_tonnage;
        let pct_error = if actual_tonnage > 0.0 {
            (error / actual_tonnage) * 100.0
        } else {
            0.0
        };
        println!("  Estimated: {:.2} t", estimated);
        println!(
            "  Error: {:+.2} t ({:+.1}%)",
            error, pct_error
        );
    }

    Ok(())
}

fn cmd_history(config: &Config, with_feedback: bool, limit: usize) -> Result<()> {
    let store = open_history_store(config)?;

    let entries = if with_feedback {
        store.entries_with_feedback()
    } else {
        store.all_entries()
    };

    println!("Analysis History");
    println!("================");
    println!("Total entries: {} (with feedback: {})", store.count(), store.feedback_count());
    println!();

    if entries.is_empty() {
        println!("No entries found.");
        return Ok(());
    }

    // Header
    println!(
        "{:<40} {:>8} {:>8} {:>8} {:>10}",
        "Image", "Est.(t)", "Act.(t)", "Err.(t)", "Date"
    );
    println!("{}", "-".repeat(78));

    for entry in entries.iter().take(limit) {
        let filename = std::path::Path::new(&entry.image_path)
            .file_name()
            .and_then(|n| n.to_str())
            .unwrap_or(&entry.image_path);

        // Truncate filename if too long
        let display_name = if filename.len() > 38 {
            format!("{}...", &filename[..35])
        } else {
            filename.to_string()
        };

        let actual_str = entry
            .actual_tonnage
            .map(|t| format!("{:.2}", t))
            .unwrap_or_else(|| "-".to_string());

        let error_str = entry
            .actual_tonnage
            .map(|actual| {
                let err = entry.estimation.estimated_tonnage - actual;
                format!("{:+.2}", err)
            })
            .unwrap_or_else(|| "-".to_string());

        let date_str = entry.analyzed_at.format("%m/%d %H:%M").to_string();

        println!(
            "{:<40} {:>8.2} {:>8} {:>8} {:>10}",
            display_name,
            entry.estimation.estimated_tonnage,
            actual_str,
            error_str,
            date_str
        );
    }

    if entries.len() > limit {
        println!();
        println!("... and {} more entries", entries.len() - limit);
    }

    Ok(())
}

fn cmd_accuracy(
    config: &Config,
    by_truck: bool,
    by_material: bool,
    detailed: bool,
) -> Result<()> {
    let store = open_history_store(config)?;
    let stats = store.accuracy_stats();

    if stats.sample_count == 0 {
        println!("No feedback data available.");
        println!("Use 'tonsuu-checker feedback <image> --actual <tonnage>' to add ground truth.");
        return Ok(());
    }

    println!("Accuracy Report");
    println!("===============");
    println!();

    print_accuracy_stats("Overall", &stats);

    if by_truck {
        println!();
        println!("By Truck Type");
        println!("-------------");
        let grouped = stats.by_truck_type();
        let mut keys: Vec<_> = grouped.keys().collect();
        keys.sort();
        for key in keys {
            if let Some(s) = grouped.get(key) {
                println!();
                print_accuracy_stats(key, s);
            }
        }
    }

    if by_material {
        println!();
        println!("By Material Type");
        println!("----------------");
        let grouped = stats.by_material_type();
        let mut keys: Vec<_> = grouped.keys().collect();
        keys.sort();
        for key in keys {
            if let Some(s) = grouped.get(key) {
                println!();
                print_accuracy_stats(key, s);
            }
        }
    }

    if detailed {
        println!();
        println!("Detailed Samples");
        println!("----------------");
        println!(
            "{:>10} {:>10} {:>10} {:>10} {:>12} {:>12}",
            "Estimated", "Actual", "Error", "Error%", "Truck", "Material"
        );
        println!("{}", "-".repeat(70));

        for sample in &stats.samples {
            println!(
                "{:>10.2} {:>10.2} {:>10.2} {:>9.1}% {:>12} {:>12}",
                sample.estimated,
                sample.actual,
                sample.error(),
                sample.percent_error(),
                truncate(&sample.truck_type, 12),
                truncate(&sample.material_type, 12)
            );
        }
    }

    Ok(())
}

fn print_accuracy_stats(label: &str, stats: &tonsuu_store::AccuracyStats) {
    println!("{} (n={})", label, stats.sample_count);
    println!("  Mean Error:     {:+.3} t", stats.mean_error);
    println!("  Mean Abs Error: {:.3} t", stats.mean_abs_error);
    println!("  RMSE:           {:.3} t", stats.rmse);
    println!("  Mean % Error:   {:.1}%", stats.mean_percent_error);
    println!(
        "  Range:          {:+.2} ~ {:+.2} t",
        stats.min_error, stats.max_error
    );
}

fn truncate(s: &str, max_len: usize) -> String {
    if s.len() > max_len {
        format!("{}...", &s[..max_len.saturating_sub(3)])
    } else {
        s.to_string()
    }
}

/// Find vehicle by license plate with fuzzy matching
#[allow(dead_code)]
fn find_vehicle_by_plate<'a>(
    vehicle_store: &'a tonsuu_store::VehicleStore,
    plate: &str,
) -> Option<&'a tonsuu_types::RegisteredVehicle> {
    // Try exact match first
    if let Some(vehicle) = vehicle_store.get_by_license_plate(plate) {
        return Some(vehicle);
    }

    // Try fuzzy match (remove spaces, normalize)
    let normalized_plate = plate.replace(' ', "").replace('\u{3000}', "").replace('-', "");
    let plate_nums: String = normalized_plate.chars().filter(|c| c.is_ascii_digit()).collect();

    for vehicle in vehicle_store.all_vehicles() {
        if let Some(ref vplate) = vehicle.license_plate {
            let normalized_vplate = vplate.replace(' ', "").replace('\u{3000}', "").replace('-', "");

            // Direct normalized match
            if normalized_plate == normalized_vplate {
                return Some(vehicle);
            }

            // Check if last 4 digits match
            let vplate_nums: String = normalized_vplate.chars().filter(|c| c.is_ascii_digit()).collect();
            if plate_nums.len() >= 4 && vplate_nums.len() >= 4 {
                let plate_last4 = &plate_nums[plate_nums.len()-4..];
                let vplate_last4 = &vplate_nums[vplate_nums.len()-4..];
                if plate_last4 == vplate_last4 {
                    return Some(vehicle);
                }
            }
        }
    }

    None
}

fn cmd_auto_collect(
    cli: &Cli,
    config: &Config,
    folder: PathBuf,
    yes: bool,
    jobs: usize,
    dry_run: bool,
    company: Option<String>,
) -> Result<()> {
    use tonsuu_types::RegisteredVehicle;

    if !folder.exists() || !folder.is_dir() {
        return Err(Error::FileNotFound(format!(
            "Folder not found: {}",
            folder.display()
        )));
    }

    println!("Scanning folder: {}", folder.display());

    // Scan for vehicle subfolders
    let vehicle_folders = scan_vehicle_folders(&folder);

    if vehicle_folders.is_empty() {
        println!("No vehicle folders found.");
        return Ok(());
    }

    println!("\nFound {} vehicle folder(s):", vehicle_folders.len());
    println!("{:<30} {:>8} {:>8}", "Folder", "車検証", "写真");
    println!("{}", "-".repeat(50));

    for vf in &vehicle_folders {
        println!(
            "{:<30} {:>8} {:>8}",
            truncate(&vf.folder_name, 28),
            vf.shaken_files.len(),
            vf.photo_files.len()
        );
    }

    if dry_run {
        println!("\n[Dry run mode - no vehicles will be registered]");
        return Ok(());
    }

    // Confirmation
    if !yes {
        println!("\nRegister {} vehicle(s)? [y/N]", vehicle_folders.len());
        let mut input = String::new();
        std::io::stdin().read_line(&mut input).ok();
        if !input.trim().eq_ignore_ascii_case("y") {
            println!("Cancelled.");
            return Ok(());
        }
    }

    // Open vehicle store
    let mut vehicle_store = open_vehicle_store(config)?;

    // Setup analyzer config
    let analyzer_config = AnalyzerConfig::default()
        .with_backend(&config.backend)
        .with_model(config.model.clone())
        .with_usage_mode(&config.usage_mode);

    // Progress bar
    let pb = ProgressBar::new(vehicle_folders.len() as u64);
    pb.set_style(
        ProgressStyle::default_bar()
            .template("{spinner:.green} [{elapsed_precise}] [{bar:40.cyan/blue}] {pos}/{len} {msg}")
            .unwrap()
            .progress_chars("#>-"),
    );

    let mut success_count = 0;
    let mut fail_count = 0;

    // Process sequentially or in parallel
    if jobs <= 1 {
        // Sequential processing
        for vf in vehicle_folders {
            pb.set_message(truncate(&vf.folder_name, 30));

            match process_vehicle_folder(&vf, &analyzer_config, cli.verbose, company.as_deref()) {
                Ok(vehicle) => {
                    if let Err(e) = vehicle_store.add_vehicle(vehicle) {
                        if cli.verbose {
                            eprintln!("  Failed to register {}: {}", vf.folder_name, e);
                        }
                        fail_count += 1;
                    } else {
                        success_count += 1;
                    }
                }
                Err(e) => {
                    if cli.verbose {
                        eprintln!("  Failed {}: {}", vf.folder_name, e);
                    }
                    fail_count += 1;
                }
            }

            pb.inc(1);
        }
    } else {
        // Parallel processing
        let results: Arc<Mutex<Vec<(String, std::result::Result<RegisteredVehicle, String>)>>> =
            Arc::new(Mutex::new(Vec::new()));
        let folders = Arc::new(vehicle_folders);
        let next_index = Arc::new(AtomicUsize::new(0));
        let backend = config.backend.clone();
        let model = config.model.clone();
        let usage_mode_str = config.usage_mode.clone();
        let verbose = cli.verbose;
        let company_arc = Arc::new(company.clone());

        let mut handles = Vec::new();
        let job_count = jobs.min(folders.len());

        for _ in 0..job_count {
            let folders = Arc::clone(&folders);
            let next_index = Arc::clone(&next_index);
            let results = Arc::clone(&results);
            let backend = backend.clone();
            let model = model.clone();
            let usage_mode_for_worker = usage_mode_str.clone();
            let pb = pb.clone();
            let company = Arc::clone(&company_arc);

            let handle = thread::spawn(move || {
                let worker_config = AnalyzerConfig::default()
                    .with_backend(&backend)
                    .with_model(model)
                    .with_usage_mode(&usage_mode_for_worker);

                loop {
                    let idx = next_index.fetch_add(1, Ordering::SeqCst);
                    if idx >= folders.len() {
                        break;
                    }

                    let vf = &folders[idx];
                    pb.set_message(truncate(&vf.folder_name, 30));

                    let result: std::result::Result<RegisteredVehicle, String> =
                        process_vehicle_folder(vf, &worker_config, verbose, company.as_deref())
                            .map_err(|e| e.to_string());

                    {
                        let mut guard = results.lock().unwrap();
                        guard.push((vf.folder_name.clone(), result));
                    }

                    pb.inc(1);
                }
            });

            handles.push(handle);
        }

        for handle in handles {
            let _ = handle.join();
        }

        // Register all vehicles
        let task_results = Arc::try_unwrap(results)
            .expect("All workers done")
            .into_inner()
            .unwrap();

        for (name, result) in task_results {
            match result {
                Ok(vehicle) => {
                    if let Err(e) = vehicle_store.add_vehicle(vehicle) {
                        if verbose {
                            eprintln!("  Failed to register {}: {}", name, e);
                        }
                        fail_count += 1;
                    } else {
                        success_count += 1;
                    }
                }
                Err(e) => {
                    if verbose {
                        eprintln!("  Failed {}: {}", name, e);
                    }
                    fail_count += 1;
                }
            }
        }
    }

    pb.finish_and_clear();

    println!("\nAuto-collect complete");
    println!("  Success: {}", success_count);
    println!("  Failed:  {}", fail_count);
    println!("  Total registered vehicles: {}", vehicle_store.count());

    Ok(())
}

/// Scanned vehicle folder information
#[derive(Debug, Clone)]
struct VehicleFolderInfo {
    folder_name: String,
    #[allow(dead_code)]
    folder_path: PathBuf,
    shaken_files: Vec<PathBuf>,
    photo_files: Vec<PathBuf>,
}

/// Scan folder for vehicle subfolders
fn scan_vehicle_folders(root: &PathBuf) -> Vec<VehicleFolderInfo> {
    let mut folders = Vec::new();

    let Ok(entries) = std::fs::read_dir(root) else {
        return folders;
    };

    for entry in entries.flatten() {
        let path = entry.path();
        if !path.is_dir() {
            continue;
        }

        let folder_name = path
            .file_name()
            .map(|n| n.to_string_lossy().to_string())
            .unwrap_or_default();

        // Skip hidden folders and special folders
        if folder_name.starts_with('.') || folder_name == "ocr_results" {
            continue;
        }

        let (shaken_files, photo_files) = scan_folder_files(&path);

        // Only include if has some files
        if !shaken_files.is_empty() || !photo_files.is_empty() {
            folders.push(VehicleFolderInfo {
                folder_name,
                folder_path: path,
                shaken_files,
                photo_files,
            });
        }
    }

    // Sort by folder name
    folders.sort_by(|a, b| a.folder_name.cmp(&b.folder_name));
    folders
}

/// Scan a folder for 車検証 and photo files (supports PDF and images)
fn scan_folder_files(folder: &PathBuf) -> (Vec<PathBuf>, Vec<PathBuf>) {
    let mut shaken_files = Vec::new();
    let mut photo_files = Vec::new();

    let image_extensions = ["jpg", "jpeg", "png", "gif", "bmp", "webp"];
    let document_extensions = ["pdf"];

    let Ok(entries) = std::fs::read_dir(folder) else {
        return (shaken_files, photo_files);
    };

    for entry in entries.flatten() {
        let path = entry.path();
        if !path.is_file() {
            continue;
        }

        let extension = path
            .extension()
            .and_then(|e| e.to_str())
            .map(|e| e.to_lowercase())
            .unwrap_or_default();

        let filename = path
            .file_name()
            .and_then(|n| n.to_str())
            .map(|n| n.to_lowercase())
            .unwrap_or_default();

        // Skip desktop.ini and other system files
        if filename == "desktop.ini" || filename.starts_with('.') {
            continue;
        }

        let is_image = image_extensions.contains(&extension.as_str());
        let is_document = document_extensions.contains(&extension.as_str());

        if !is_image && !is_document {
            continue;
        }

        // Detect 車検証 files by filename patterns
        if filename.contains("車検") || filename.contains("shaken")
            || filename.contains("certificate") || filename.contains("registration")
            || filename.contains("検査") || filename.starts_with("cert")
        {
            shaken_files.push(path);
        } else if filename.contains("写真") || filename.contains("photo")
            || filename.contains("picture") || filename.contains("image")
            || is_image
        {
            // Photo files
            photo_files.push(path);
        } else if is_document {
            // Other PDFs - check if it's a photo PDF by name
            if !filename.contains("車検") {
                photo_files.push(path);
            }
        }
    }

    // Sort
    shaken_files.sort();
    photo_files.sort();

    (shaken_files, photo_files)
}

/// Process a single vehicle folder
fn process_vehicle_folder(
    vf: &VehicleFolderInfo,
    _config: &AnalyzerConfig,
    verbose: bool,
    company: Option<&str>,
) -> Result<RegisteredVehicle> {
    use cli_ai_analyzer::{analyze, AnalyzeOptions, Backend};

    // Need at least a shaken file for capacity
    if vf.shaken_files.is_empty() {
        return Err(Error::AnalysisFailed("No 車検証 file found".to_string()));
    }

    // Analyze 車検証
    let shaken_path = &vf.shaken_files[0];
    if verbose {
        eprintln!("  Analyzing 車検証: {}", shaken_path.display());
    }

    let prompt = r#"この画像は日本の自動車検査証（車検証）です。以下の情報を抽出してください。

抽出する項目:
1. 車名（例: 日野, いすゞ, 三菱ふそう, UD）
2. 型式（例: プロフィア, ギガ, スーパーグレート）
3. 最大積載量（kg単位の数値）
4. 車両番号（ナンバープレート）

以下のJSON形式で回答してください:
{
  "vehicleName": "車名 型式",
  "maxCapacityKg": 10000,
  "licensePlate": "品川 100 あ 1234"
}

注意:
- 最大積載量は必ずkg単位の数値で返してください
- 読み取れない項目はnullとしてください
- 車検証でない画像の場合は全てnullとしてください
"#;

    let options = AnalyzeOptions::default()
        .with_backend(Backend::Gemini)
        .json();

    let response = analyze(prompt, &[shaken_path.clone()], options)
        .map_err(|e| Error::AnalysisFailed(format!("AI error: {}", e)))?;

    // Parse response
    #[derive(serde::Deserialize)]
    #[serde(rename_all = "camelCase")]
    struct ShakenResult {
        vehicle_name: Option<String>,
        max_capacity_kg: Option<f64>,
        license_plate: Option<String>,
    }

    let json_str = extract_json_response(&response);
    let shaken: ShakenResult = serde_json::from_str(&json_str)
        .map_err(|e| Error::AnalysisFailed(format!("JSON parse error: {}", e)))?;

    let vehicle_name = shaken.vehicle_name.unwrap_or_else(|| vf.folder_name.clone());
    let max_capacity = shaken.max_capacity_kg
        .map(|kg| kg / 1000.0)
        .ok_or_else(|| Error::AnalysisFailed("Could not detect max capacity".to_string()))?;

    // Get photo path
    let photo_path = vf.photo_files.first()
        .ok_or_else(|| Error::AnalysisFailed("No photo file found".to_string()))?;

    // Create thumbnail
    let thumbnail = create_thumbnail_from_path(photo_path);

    // Create vehicle
    let mut vehicle = RegisteredVehicle::new(vehicle_name, max_capacity)
        .with_image(photo_path.display().to_string(), thumbnail);

    if let Some(plate) = shaken.license_plate {
        vehicle = vehicle.with_license_plate(plate);
    }

    if let Some(company_name) = company {
        vehicle.company = Some(company_name.to_string());
    }

    vehicle.notes = Some(format!("Auto-collected from: {}", vf.folder_name));

    Ok(vehicle)
}

/// Extract JSON from AI response
fn extract_json_response(response: &str) -> String {
    let response = response.trim();

    if response.starts_with("```json") {
        if let Some(end) = response.rfind("```") {
            let start = response.find('\n').unwrap_or(7) + 1;
            if start < end {
                return response[start..end].trim().to_string();
            }
        }
    }

    if response.starts_with("```") {
        if let Some(end) = response.rfind("```") {
            let start = response.find('\n').unwrap_or(3) + 1;
            if start < end {
                return response[start..end].trim().to_string();
            }
        }
    }

    if let Some(start) = response.find('{') {
        if let Some(end) = response.rfind('}') {
            if start < end {
                return response[start..=end].to_string();
            }
        }
    }

    response.to_string()
}

/// Create thumbnail from file path
fn create_thumbnail_from_path(path: &PathBuf) -> Option<String> {
    use base64::{engine::general_purpose::STANDARD, Engine};
    use std::fs::File;
    use std::io::Read;

    // Check if it's a PDF - for now skip thumbnail for PDFs
    let ext = path.extension()
        .and_then(|e| e.to_str())
        .map(|e| e.to_lowercase())
        .unwrap_or_default();

    if ext == "pdf" {
        // PDFs need special handling - return None for now
        return None;
    }

    let mut file = File::open(path).ok()?;
    let mut buffer = Vec::new();
    file.read_to_end(&mut buffer).ok()?;

    Some(STANDARD.encode(&buffer))
}

/// Backup JSON stock entry from TonSuuChecker app
#[derive(Debug, Deserialize)]
#[serde(rename_all = "camelCase")]
struct BackupStockEntry {
    id: String,
    timestamp: i64,
    #[serde(default)]
    base64_images: Vec<String>,
    #[serde(default)]
    max_capacity: Option<f64>,
    #[serde(default)]
    actual_tonnage: Option<f64>,
    #[serde(default)]
    estimations: Vec<BackupEstimation>,
}

/// Backup estimation from TonSuuChecker app
#[allow(dead_code)]
#[derive(Debug, Deserialize)]
#[serde(rename_all = "camelCase")]
struct BackupEstimation {
    #[serde(default)]
    is_target_detected: bool,
    #[serde(default)]
    truck_type: String,
    #[serde(default)]
    material_type: String,
    #[serde(default)]
    estimated_volume_m3: f64,
    #[serde(default)]
    estimated_tonnage: f64,
    #[serde(default)]
    estimated_max_capacity: Option<f64>,
    #[serde(default)]
    confidence_score: f64,
    #[serde(default)]
    reasoning: String,
    #[serde(default)]
    license_plate: Option<String>,
}

/// Backup JSON structure from TonSuuChecker app
#[derive(Debug, Deserialize)]
struct BackupJson {
    #[serde(default)]
    version: i32,
    #[serde(default)]
    stock: Vec<BackupStockEntry>,
}

fn cmd_import(config: &Config, file: PathBuf, dry_run: bool) -> Result<()> {
    use chrono::{TimeZone, Utc};

    if !file.exists() {
        return Err(Error::FileNotFound(format!(
            "Backup file not found: {}",
            file.display()
        )));
    }

    println!("Reading backup file: {}", file.display());

    // Read and parse backup JSON
    let content = std::fs::read_to_string(&file)?;
    let backup: BackupJson = serde_json::from_str(&content)
        .map_err(|e| Error::AnalysisFailed(format!("Failed to parse backup JSON: {}", e)))?;

    println!("Backup version: {}", backup.version);
    println!("Total entries in backup: {}", backup.stock.len());

    if backup.stock.is_empty() {
        println!("No entries to import.");
        return Ok(());
    }

    // Open store
    let mut store = open_history_store(config)?;

    let mut imported = 0;
    let mut skipped = 0;
    let mut errors = 0;

    for entry in &backup.stock {
        // Use ID as image_hash for duplicate checking
        let image_hash = entry.id.clone();

        // Check if already exists
        if store.has_entry(&image_hash) {
            skipped += 1;
            continue;
        }

        // Convert timestamp (milliseconds) to DateTime
        let analyzed_at = Utc
            .timestamp_millis_opt(entry.timestamp)
            .single()
            .unwrap_or_else(Utc::now);

        // Get first estimation if available
        let estimation = if let Some(est) = entry.estimations.first() {
            EstimationResult {
                is_target_detected: est.is_target_detected,
                truck_type: est.truck_type.clone(),
                license_plate: est.license_plate.clone(),
                material_type: est.material_type.clone(),
                height: None,
                packing_density: None,
                fill_ratio_l: None,
                fill_ratio_w: None,
                fill_ratio_z: None,
                estimated_volume_m3: est.estimated_volume_m3,
                estimated_tonnage: est.estimated_tonnage,
                confidence_score: est.confidence_score,
                reasoning: est.reasoning.clone(),
                material_breakdown: Vec::new(),
                ensemble_count: None,
            }
        } else {
            // No estimation, create default
            EstimationResult::default()
        };

        // Create HistoryEntry
        let history_entry = HistoryEntry {
            image_path: format!("[imported from backup: {}]", entry.id),
            image_hash,
            estimation,
            actual_tonnage: entry.actual_tonnage,
            max_capacity: entry.max_capacity,
            analyzed_at,
            feedback_at: entry.actual_tonnage.map(|_| analyzed_at),
            notes: Some("Imported from TonSuuChecker app backup".to_string()),
            thumbnail_base64: entry.base64_images.first().cloned(),
        };

        if dry_run {
            println!(
                "  [DRY RUN] Would import: {} - {:.2}t ({})",
                &history_entry.image_hash[..8],
                history_entry.estimation.estimated_tonnage,
                history_entry.estimation.truck_type
            );
            imported += 1;
        } else {
            match store.add_entry(history_entry) {
                Ok(true) => {
                    imported += 1;
                }
                Ok(false) => {
                    skipped += 1;
                }
                Err(e) => {
                    eprintln!("  Error importing {}: {}", entry.id, e);
                    errors += 1;
                }
            }
        }
    }

    println!();
    if dry_run {
        println!("[DRY RUN] Import summary:");
        println!("  Would import: {}", imported);
        println!("  Would skip (duplicates): {}", skipped);
    } else {
        println!("Import complete:");
        println!("  Imported: {}", imported);
        println!("  Skipped (duplicates): {}", skipped);
        println!("  Errors: {}", errors);
        println!("  Total entries in store: {}", store.count());
    }

    Ok(())
}

/// Check AI backend status and rate limits
fn cmd_stats(cli: &Cli) -> Result<()> {
    let backend = cli.backend.as_deref().unwrap_or("gemini");

    println!("Checking {} status...", backend);

    match backend.to_lowercase().as_str() {
        "gemini" => {
            match check_gemini_status(None) {
                Ok(stats) => {
                    if stats.is_available {
                        println!("✓ Gemini API is available");
                    } else {
                        println!("✗ Gemini API is not available");
                        if let Some(msg) = &stats.rate_limit_message {
                            println!("  Rate limit: {}", msg);
                        }
                        if let Some(retry) = stats.retry_after_seconds {
                            println!("  Retry after: {} seconds", retry);
                        }
                    }
                    if cli.verbose {
                        println!("\nRaw response:\n{}", stats.raw_response);
                    }
                }
                Err(e) => {
                    println!("✗ Error checking Gemini status: {}", e);
                }
            }
        }
        "claude" => {
            println!("Claude status check not yet implemented");
            println!("Hint: Use 'claude doctor' to check Claude CLI status");
        }
        _ => {
            println!("Unknown backend: {}", backend);
        }
    }

    Ok(())
}

/// Check for overloaded vehicles
fn cmd_check_overload(csv_path: PathBuf, vehicles_path: PathBuf, output_format: OutputFormat) -> Result<()> {
    // Validate file paths
    if !csv_path.exists() {
        return Err(Error::FileNotFound(format!(
            "CSV file not found: {}",
            csv_path.display()
        )));
    }
    if !vehicles_path.exists() {
        return Err(Error::FileNotFound(format!(
            "Vehicles file not found: {}",
            vehicles_path.display()
        )));
    }

    // Load data
    println!("Loading weighing slips from: {}", csv_path.display());
    let slips = load_slips_from_csv(&csv_path)
        .map_err(|e| Error::AnalysisFailed(format!("Failed to load slips: {}", e)))?;
    println!("  Loaded {} slips", slips.len());

    println!("Loading vehicle master from: {}", vehicles_path.display());
    let vehicles = load_vehicles_from_csv(&vehicles_path)
        .map_err(|e| Error::AnalysisFailed(format!("Failed to load vehicles: {}", e)))?;
    println!("  Loaded {} vehicles", vehicles.len());

    // Run overload check
    println!("\nChecking for overloads...\n");
    let results = check_overloads(&slips, &vehicles);

    // Output results
    match output_format {
        OutputFormat::Json => {
            let json = serde_json::to_string_pretty(&results)?;
            println!("{}", json);
        }
        OutputFormat::Table => {
            let report = generate_overload_report(&results);
            println!("{}", report);
        }
    }

    // Return success or error based on overload count
    let overload_count = results.iter().filter(|r| r.is_overloaded).count();
    if overload_count > 0 {
        eprintln!("\n警告: {}件の過積載が検出されました", overload_count);
    }

    Ok(())
}
