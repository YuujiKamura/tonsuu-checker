//! Command handlers

use crate::analyzer::cache::Cache;
use crate::analyzer::{analyze_image, analyze_image_ensemble, AnalyzerConfig};
use crate::cli::{Cli, Commands, OutputFormat};
use crate::config::Config;
use crate::constants::get_truck_spec;
use crate::error::{Error, Result};
use crate::export::export_to_excel;
use crate::output::output_result;
use crate::scanner::{scan_directory, validate_image};
use crate::types::{AnalysisEntry, BatchResults, EstimationResult, LoadGrade};
use chrono::Utc;
use indicatif::{MultiProgress, ProgressBar, ProgressStyle};
use std::path::PathBuf;
use std::sync::atomic::{AtomicUsize, Ordering};
use std::sync::{Arc, Mutex};
use std::thread;

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

    match &cli.command {
        Commands::Analyze {
            image,
            no_cache,
            ensemble,
        } => {
            // Use CLI ensemble if specified, otherwise config value
            let ensemble_count = ensemble.unwrap_or(config.ensemble_count);
            // Cache disabled if: --no-cache OR config.cache_enabled=false
            let use_cache = !no_cache && config.cache_enabled;
            let output_format = cli.format.unwrap_or(config.output_format);
            cmd_analyze(&cli, &config, image.clone(), use_cache, ensemble_count, output_format)
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
            reset,
        } => cmd_config(
            *show,
            set_backend.clone(),
            set_model.clone(),
            *set_cache,
            *set_output,
            *set_ensemble,
            *reset,
        ),

        Commands::Cache { clear, stats } => cmd_cache(&config, *clear, *stats),
    }
}

fn cmd_analyze(
    cli: &Cli,
    config: &Config,
    image: PathBuf,
    use_cache: bool,
    ensemble: u32,
    output_format: OutputFormat,
) -> Result<()> {
    // Validate image
    validate_image(&image)?;

    // Setup analyzer config
    let analyzer_config = AnalyzerConfig::default()
        .with_backend(&config.backend)
        .with_model(config.model.clone());

    // Initialize cache once if enabled
    let cache = if use_cache {
        Some(Cache::new(config.cache_dir()?)?)
    } else {
        None
    };

    // Check cache first
    if let Some(ref cache) = cache {
        if let Ok(Some(cached)) = cache.get(&image) {
            if cli.verbose {
                eprintln!("Using cached result");
            }
            output_result(output_format, &cached)?;
            return Ok(());
        }
    }

    // Run analysis
    if cli.verbose {
        eprintln!("Analyzing image: {}", image.display());
    }

    let result = if ensemble > 1 {
        analyze_image_ensemble(&image, &analyzer_config, ensemble)?
    } else {
        analyze_image(&image, &analyzer_config)?
    };

    // Cache result
    if let Some(ref cache) = cache {
        let _ = cache.set(&image, &result);
    }

    // Output result
    output_result(output_format, &result)?;

    Ok(())
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

    // Setup shared state
    let cache_dir = if use_cache {
        Some(config.cache_dir()?)
    } else {
        None
    };
    let backend = config.backend.clone();
    let model = config.model.clone();

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
        let cache_dir = cache_dir.clone();
        let backend = backend.clone();
        let model = model.clone();
        let pb = main_pb.clone();

        let handle = thread::spawn(move || {
            // Setup analyzer config for this worker
            let analyzer_config = AnalyzerConfig::default()
                .with_backend(&backend)
                .with_model(model);

            // Setup cache for this worker (only if caching enabled and dir available)
            let cache = cache_dir.and_then(|dir| Cache::new(dir).ok());

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

                // Check cache first (only if caching enabled)
                let result = if let Some(ref cache) = cache {
                    if let Ok(Some(cached)) = cache.get(image) {
                        Ok(cached)
                    } else {
                        analyze_image(image, &analyzer_config).map_err(|e| e.to_string())
                    }
                } else {
                    analyze_image(image, &analyzer_config).map_err(|e| e.to_string())
                };

                // Cache successful result (only if caching enabled)
                if let Ok(ref res) = result {
                    if let Some(ref cache) = cache {
                        let _ = cache.set(image, res);
                    }
                }

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
                // Calculate grade
                let grade = if let Some(max_cap) = result.estimated_max_capacity {
                    Some(LoadGrade::from_ratio(result.estimated_tonnage / max_cap))
                } else if let Some(spec) = get_truck_spec(&result.truck_type) {
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
        return Err(Error::Cache(crate::error::CacheError::IoError(
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

