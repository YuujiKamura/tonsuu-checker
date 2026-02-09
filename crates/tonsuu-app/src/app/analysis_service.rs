//! Analysis Service - Core Use Case for Truck Image Analysis
//!
//! This service orchestrates the complete analysis workflow:
//! 1. Validate input image
//! 2. Check cache for existing results
//! 3. Detect license plate (YOLO or API)
//! 4. Match against registered vehicles
//! 5. Call vision module for AI analysis
//! 6. Calculate weight using domain services
//! 7. Store results in history
//! 8. Return analysis result

use crate::config::Config;
use crate::scanner::validate_image;
use thiserror::Error;
use tonsuu_store::{Store, VehicleStore};
use tonsuu_types::{Error, EstimationResult, LoadGrade, RegisteredVehicle, TruckClass};
use tonsuu_vision::{
    analyze_image_box_overlay, analyze_image_staged, AnalyzerConfig, Cache, ProgressCallback,
    StagedAnalysisOptions,
};
use std::path::Path;

/// Errors specific to the analysis service
#[derive(Debug, Error)]
pub enum AnalysisServiceError {
    #[error("Image validation failed: {0}")]
    InvalidImage(String),

    #[error("Analysis failed: {0}")]
    AnalysisFailed(String),

    #[error("Cache error: {0}")]
    CacheError(String),

    #[error("Store error: {0}")]
    StoreError(String),

    #[error("Configuration error: {0}")]
    ConfigError(String),
}

impl From<Error> for AnalysisServiceError {
    fn from(err: Error) -> Self {
        match err {
            Error::FileNotFound(msg) | Error::InvalidImageFormat(msg) => {
                AnalysisServiceError::InvalidImage(msg)
            }
            Error::AnalysisFailed(msg) => AnalysisServiceError::AnalysisFailed(msg),
            Error::Cache(e) => AnalysisServiceError::CacheError(e.to_string()),
            Error::Config(e) => AnalysisServiceError::ConfigError(e.to_string()),
            _ => AnalysisServiceError::AnalysisFailed(err.to_string()),
        }
    }
}

/// Options for analysis
#[derive(Debug, Clone, Default)]
pub struct AnalysisOptions {
    /// Manual license plate override
    pub manual_plate: Option<String>,

    /// Skip YOLO detection, use specified truck class
    pub truck_class_override: Option<TruckClass>,

    /// Filter vehicles by company name
    pub company_filter: Option<String>,

    /// Number of ensemble samples
    pub ensemble_count: u32,

    /// Whether to use cache
    pub use_cache: bool,

    /// Verbose output (for progress callbacks)
    pub verbose: bool,

    /// Material type pre-info (e.g., "As殻", "Co殻", "土砂")
    pub material_type: Option<String>,

    /// Truck type pre-info (e.g., "4tダンプ", "10tダンプ")
    pub truck_type_hint: Option<String>,

    /// Karte JSON (known values; null means estimate)
    pub karte_json: Option<String>,
}

impl AnalysisOptions {
    pub fn new() -> Self {
        Self {
            ensemble_count: 1,
            use_cache: true,
            ..Default::default()
        }
    }

    pub fn with_manual_plate(mut self, plate: String) -> Self {
        self.manual_plate = Some(plate);
        self
    }

    pub fn with_truck_class(mut self, class: TruckClass) -> Self {
        self.truck_class_override = Some(class);
        self
    }

    pub fn with_company_filter(mut self, company: String) -> Self {
        self.company_filter = Some(company);
        self
    }

    pub fn with_ensemble_count(mut self, count: u32) -> Self {
        self.ensemble_count = count.max(1);
        self
    }

    pub fn with_cache(mut self, enabled: bool) -> Self {
        self.use_cache = enabled;
        self
    }

    pub fn with_verbose(mut self, verbose: bool) -> Self {
        self.verbose = verbose;
        self
    }

    pub fn with_material_type(mut self, material_type: String) -> Self {
        self.material_type = Some(material_type);
        self
    }

    pub fn with_truck_type_hint(mut self, truck_type: String) -> Self {
        self.truck_type_hint = Some(truck_type);
        self
    }

    pub fn with_karte_json(mut self, karte_json: String) -> Self {
        self.karte_json = Some(karte_json);
        self
    }
}

/// Result of the analysis containing estimation and matched vehicle info
#[derive(Debug, Clone)]
pub struct AnalysisResult {
    /// The AI estimation result
    pub estimation: EstimationResult,

    /// Matched vehicle (if any)
    pub matched_vehicle: Option<RegisteredVehicle>,

    /// Load grade (if max_capacity known)
    #[allow(dead_code)]
    pub load_grade: Option<LoadGrade>,

    /// Load ratio (estimated / max_capacity)
    #[allow(dead_code)]
    pub load_ratio: Option<f64>,

    /// Whether result came from cache
    pub from_cache: bool,
}

impl AnalysisResult {
    /// Get the max capacity from matched vehicle
    #[allow(dead_code)]
    pub fn max_capacity(&self) -> Option<f64> {
        self.matched_vehicle.as_ref().map(|v| v.max_capacity)
    }
}

/// Main entry point: Analyze a truck image
///
/// This is the primary use case that orchestrates the complete analysis workflow.
///
/// # Arguments
/// * `image_path` - Path to the image file to analyze
/// * `config` - Application configuration
/// * `options` - Analysis options (cache, ensemble, filters, etc.)
/// * `progress` - Optional progress callback for verbose output
///
/// # Returns
/// * `AnalysisResult` containing estimation, matched vehicle, and load grade
pub fn analyze_truck_image(
    image_path: &Path,
    config: &Config,
    options: &AnalysisOptions,
    progress: Option<ProgressCallback>,
) -> std::result::Result<AnalysisResult, AnalysisServiceError> {
    // Step 1: Validate image
    validate_image(image_path)?;

    // Step 2: Initialize stores and cache
    let store = Store::open(config.store_dir().map_err(|e| {
        AnalysisServiceError::StoreError(format!("Failed to open store: {}", e))
    })?)?;

    let vehicle_store = VehicleStore::open(config.store_dir().map_err(|e| {
        AnalysisServiceError::StoreError(format!("Failed to open vehicle store: {}", e))
    })?)?;

    let cache = if options.use_cache {
        config
            .cache_dir()
            .ok()
            .and_then(|dir| Cache::new(dir).ok())
    } else {
        None
    };

    // Step 3: Check cache (only if no manual overrides)
    if options.manual_plate.is_none() && options.truck_class_override.is_none() {
        if let Some(ref cache) = cache {
            if let Ok(Some(cached)) = cache.get(image_path) {
                let matched = cached
                    .license_plate
                    .as_ref()
                    .and_then(|plate| find_vehicle_by_plate(&vehicle_store, plate));

                let (load_grade, load_ratio) = calculate_load_info(&cached, matched.as_ref());

                return Ok(AnalysisResult {
                    estimation: cached,
                    matched_vehicle: matched,
                    load_grade,
                    load_ratio,
                    from_cache: true,
                });
            }
        }
    }

    // Step 4: Find matched vehicle
    let matched_vehicle = find_matched_vehicle(
        &vehicle_store,
        options.manual_plate.as_deref(),
        options.company_filter.as_deref(),
    );

    // Step 5: Determine truck class
    let truck_class = options
        .truck_class_override
        .or_else(|| matched_vehicle.as_ref().map(|v| v.truck_class()));

    // Step 6: Run analysis
    let analyzer_config = AnalyzerConfig::default()
        .with_backend(&config.backend)
        .with_model(config.model.clone())
        .with_usage_mode(&config.usage_mode);

    let estimation = if options.karte_json.is_some() {
        // Karte path: use legacy staged analysis (karte is multi-param based)
        let staged_options = StagedAnalysisOptions {
            truck_class,
            ensemble_count: options.ensemble_count.max(1),
            truck_type_hint: options.truck_type_hint.clone(),
            material_type: options.material_type.clone(),
            karte_json: options.karte_json.clone(),
        };

        analyze_image_staged(
            image_path,
            &analyzer_config,
            &staged_options,
            &store,
            progress,
        )?
    } else {
        // Box-overlay pipeline (default, higher accuracy)
        // Priority: Step 5 resolved truck_class > CLI hint > default "4t"
        // TruckClass::label() returns "2t"/"4t"/"10t" which match prompt-spec.json truckSpecs keys
        // Caching is handled by Steps 3 (check) and 8 (store) above, applying to both paths
        let tc_label = truck_class.map(|tc| tc.label().to_string());
        let truck_class_str = tc_label.as_deref()
            .or(options.truck_type_hint.as_deref())
            .unwrap_or("4t");
        let material_type_str = options.material_type.as_deref().unwrap_or("As殻");
        let ensemble_count = options.ensemble_count.max(1) as usize;

        analyze_image_box_overlay(
            image_path,
            &analyzer_config,
            truck_class_str,
            material_type_str,
            ensemble_count,
            progress,
        )?
    };

    // Step 7: Calculate load info
    let (load_grade, load_ratio) = calculate_load_info(&estimation, matched_vehicle.as_ref());

    // Step 8: Cache result
    if let Some(ref cache) = cache {
        let _ = cache.set(image_path, &estimation);
    }

    // Step 9: Save to history
    let mut store_mut = Store::open(config.store_dir().map_err(|e| {
        AnalysisServiceError::StoreError(format!("Failed to open store: {}", e))
    })?)?;

    let _ = store_mut.add_analysis_with_capacity(
        image_path,
        estimation.clone(),
        matched_vehicle.as_ref().map(|v| v.max_capacity),
        None,
    );

    Ok(AnalysisResult {
        estimation,
        matched_vehicle,
        load_grade,
        load_ratio,
        from_cache: false,
    })
}

/// Simplified version without progress callback
#[allow(dead_code)]
pub fn analyze_truck_image_simple(
    image_path: &Path,
    config: &Config,
    options: &AnalysisOptions,
) -> std::result::Result<AnalysisResult, AnalysisServiceError> {
    analyze_truck_image(image_path, config, options, None)
}

/// Quick analysis with default options
#[allow(dead_code)]
pub fn analyze_quick(
    image_path: &Path,
    config: &Config,
) -> std::result::Result<AnalysisResult, AnalysisServiceError> {
    analyze_truck_image(image_path, config, &AnalysisOptions::new(), None)
}

/// Find vehicle by license plate with fuzzy matching
fn find_vehicle_by_plate(vehicle_store: &VehicleStore, plate: &str) -> Option<RegisteredVehicle> {
    // Try exact match first
    if let Some(vehicle) = vehicle_store.get_by_license_plate(plate) {
        return Some(vehicle.clone());
    }

    // Try fuzzy match (remove spaces, normalize)
    let normalized_plate = plate
        .replace(' ', "")
        .replace('\u{3000}', "")
        .replace('-', "");
    let plate_nums: String = normalized_plate
        .chars()
        .filter(|c| c.is_ascii_digit())
        .collect();

    for vehicle in vehicle_store.all_vehicles() {
        if let Some(ref vplate) = vehicle.license_plate {
            let normalized_vplate = vplate
                .replace(' ', "")
                .replace('\u{3000}', "")
                .replace('-', "");

            // Direct normalized match
            if normalized_plate == normalized_vplate {
                return Some(vehicle.clone());
            }

            // Check if last 4 digits match
            let vplate_nums: String = normalized_vplate
                .chars()
                .filter(|c| c.is_ascii_digit())
                .collect();
            if plate_nums.len() >= 4 && vplate_nums.len() >= 4 {
                let plate_last4 = &plate_nums[plate_nums.len() - 4..];
                let vplate_last4 = &vplate_nums[vplate_nums.len() - 4..];
                if plate_last4 == vplate_last4 {
                    return Some(vehicle.clone());
                }
            }
        }
    }

    None
}

/// Find matched vehicle based on manual plate or detected plate
fn find_matched_vehicle(
    vehicle_store: &VehicleStore,
    manual_plate: Option<&str>,
    _company_filter: Option<&str>,
) -> Option<RegisteredVehicle> {
    if let Some(plate) = manual_plate {
        return find_vehicle_by_plate(vehicle_store, plate);
    }
    None
}

/// Calculate load grade and ratio from estimation and matched vehicle
fn calculate_load_info(
    estimation: &EstimationResult,
    matched_vehicle: Option<&RegisteredVehicle>,
) -> (Option<LoadGrade>, Option<f64>) {
    if let Some(vehicle) = matched_vehicle {
        let ratio = estimation.estimated_tonnage / vehicle.max_capacity;
        let grade = LoadGrade::from_ratio(ratio);
        (Some(grade), Some(ratio))
    } else {
        (None, None)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_analysis_options_builder() {
        let options = AnalysisOptions::new()
            .with_manual_plate("品川 100 あ 1234".to_string())
            .with_ensemble_count(3)
            .with_cache(false);

        assert_eq!(options.manual_plate, Some("品川 100 あ 1234".to_string()));
        assert_eq!(options.ensemble_count, 3);
        assert!(!options.use_cache);
    }

    #[test]
    fn test_calculate_load_info() {
        let estimation = EstimationResult {
            estimated_tonnage: 8.5,
            ..Default::default()
        };

        let vehicle = RegisteredVehicle::new("Test".to_string(), 10.0);

        let (grade, ratio) = calculate_load_info(&estimation, Some(&vehicle));

        assert!(grade.is_some());
        assert!(ratio.is_some());
        assert!((ratio.unwrap() - 0.85).abs() < 0.01);
    }
}
