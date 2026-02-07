//! Vision module - AI-powered image analysis for tonnage estimation
//!
//! This module provides:
//! - AI-based cargo analysis using multiple backends (Gemini, Claude, Codex)
//! - License plate recognition using YOLO
//! - Vehicle registration certificate (shaken) analysis
//! - Result caching for performance optimization

pub mod ai;
pub mod cache;
pub mod plate_recognizer;
pub mod volume_estimator;

// Re-export main types for convenience
pub use ai::prompts::{
    build_analysis_prompt,
    build_estimation_prompt,
    build_karte_prompt,
    build_staged_analysis_prompt, GradedReferenceItem,
};
pub use cache::Cache;
#[allow(unused_imports)]
pub use volume_estimator::analyze_shaken;

use crate::error::{Error, Result};
use crate::store::{GradedHistoryEntry, Store};
use crate::types::{EstimationResult, TruckClass};
use cli_ai_analyzer::{analyze, AnalyzeOptions, AnalysisSession, Backend, UsageMode};
use std::path::Path;

/// Analyzer configuration
#[derive(Debug, Clone)]
pub struct AnalyzerConfig {
    pub backend: Backend,
    pub model: Option<String>,
    pub usage_mode: UsageMode,
}

impl Default for AnalyzerConfig {
    fn default() -> Self {
        Self {
            backend: Backend::Gemini,
            model: None,
            usage_mode: UsageMode::TimeBasedQuota,
        }
    }
}

impl AnalyzerConfig {
    pub fn with_backend(mut self, backend: &str) -> Self {
        self.backend = match backend.to_lowercase().as_str() {
            "claude" => Backend::Claude,
            "codex" => Backend::Codex,
            _ => Backend::Gemini,
        };
        self
    }

    pub fn with_model(mut self, model: Option<String>) -> Self {
        self.model = model;
        self
    }

    pub fn with_usage_mode(mut self, usage_mode: &str) -> Self {
        self.usage_mode = match usage_mode {
            "pay_per_use" => UsageMode::PayPerUse,
            _ => UsageMode::TimeBasedQuota,
        };
        self
    }
}

/// Analyze a single image and return estimation result
pub fn analyze_image(image_path: &Path, config: &AnalyzerConfig) -> Result<EstimationResult> {
    // Build prompt
    let prompt = build_analysis_prompt();

    // Configure options
    let mut options = if let Some(ref model) = config.model {
        AnalyzeOptions::with_model(model)
    } else {
        AnalyzeOptions::default()
    };

    options = options.with_backend(config.backend).json().with_usage_mode(config.usage_mode);

    // Call AI analyzer
    let response = analyze(&prompt, &[image_path.to_path_buf()], options)?;

    // Parse response
    parse_response(&response)
}

/// Analyze image using 2-step approach.
///
/// Step 1: Estimate height + truck/material (focused attention on height).
/// Step 2: Estimate remaining parameters with height locked in.
///
/// Uses `AnalysisSession` to keep the Gemini session alive so the image
/// is uploaded only once (step 2 uses `--resume 0`).
pub fn analyze_image_2step(image_path: &Path, config: &AnalyzerConfig) -> Result<EstimationResult> {
    use crate::vision::ai::prompts::{build_step1_height_prompt, build_step2_rest_prompt};

    let make_options = || {
        let mut opts = if let Some(ref model) = config.model {
            AnalyzeOptions::with_model(model)
        } else {
            AnalyzeOptions::default()
        };
        opts = opts.with_backend(config.backend).json().with_usage_mode(config.usage_mode);
        opts
    };

    let mut session = AnalysisSession::new(make_options())
        .map_err(|e| Error::AnalysisFailed(format!("Session creation failed: {}", e)))?;

    // Step 1: height + identification (uploads image)
    let prompt1 = build_step1_height_prompt();
    let response1 = session.first_turn(&prompt1, &[image_path.to_path_buf()])
        .map_err(|e| Error::AnalysisFailed(format!("Step 1 failed: {}", e)))?;
    let step1: EstimationResult = parse_response(&response1)?;

    let height = step1.height.unwrap_or(0.4);
    let truck_type = if step1.truck_type.is_empty() { "?" } else { &step1.truck_type };
    let material_type = if step1.material_type.is_empty() { "?" } else { &step1.material_type };

    // Step 2: remaining parameters with height locked (resume, no re-upload)
    let prompt2 = build_step2_rest_prompt(height, truck_type, material_type);
    let response2 = session.next_turn(&prompt2)
        .map_err(|e| Error::AnalysisFailed(format!("Step 2 failed: {}", e)))?;
    let step2: EstimationResult = parse_response(&response2)?;

    // Merge: use step1's height/truck/material, step2's everything else
    let mut result = step2;
    result.height = Some(height);
    result.truck_type = step1.truck_type;
    result.material_type = step1.material_type;
    result.is_target_detected = step1.is_target_detected;

    // Calculate volume and tonnage from merged parameters
    if result.estimated_volume_m3 == 0.0 || result.estimated_tonnage == 0.0 {
        calculate_volume_and_tonnage(&mut result);
    }

    Ok(result)
}

/// Analyze image using 3-step approach.
///
/// Step 1: Height ONLY (maximum attention).
/// Step 2: Area + slope + truck/material identification (given height).
/// Step 3: Fill ratios + packing density (given height + area).
///
/// Uses `AnalysisSession` to keep the Gemini session alive so the image
/// is uploaded only once (steps 2-3 use `--resume 0`).
pub fn analyze_image_3step(image_path: &Path, config: &AnalyzerConfig) -> Result<EstimationResult> {
    use crate::vision::ai::prompts::{build_step1_height_only_prompt, build_step2_area_prompt, build_step3_fill_prompt};

    let make_options = || {
        let mut opts = if let Some(ref model) = config.model {
            AnalyzeOptions::with_model(model)
        } else {
            AnalyzeOptions::default()
        };
        opts = opts.with_backend(config.backend).json().with_usage_mode(config.usage_mode);
        opts
    };

    let mut session = AnalysisSession::new(make_options())
        .map_err(|e| Error::AnalysisFailed(format!("Session creation failed: {}", e)))?;

    // Step 1: height only (uploads image)
    let prompt1 = build_step1_height_only_prompt();
    let response1 = session.first_turn(&prompt1, &[image_path.to_path_buf()])
        .map_err(|e| Error::AnalysisFailed(format!("Step 1 failed: {}", e)))?;
    let step1: EstimationResult = parse_response(&response1)?;
    let height = step1.height.unwrap_or(0.4);

    // Step 2: area + slope + identification (resume, no re-upload)
    let prompt2 = build_step2_area_prompt(height);
    let response2 = session.next_turn(&prompt2)
        .map_err(|e| Error::AnalysisFailed(format!("Step 2 failed: {}", e)))?;
    let step2: EstimationResult = parse_response(&response2)?;
    let upper_area = step2.upper_area.unwrap_or(0.5);

    // Step 3: fill ratios + packing (resume, no re-upload)
    let prompt3 = build_step3_fill_prompt(height, upper_area);
    let response3 = session.next_turn(&prompt3)
        .map_err(|e| Error::AnalysisFailed(format!("Step 3 failed: {}", e)))?;
    let step3: EstimationResult = parse_response(&response3)?;

    // Merge all steps
    let mut result = EstimationResult::default();
    result.is_target_detected = true;
    result.height = Some(height);
    result.truck_type = step2.truck_type;
    result.material_type = step2.material_type;
    result.upper_area = Some(upper_area);
    result.slope = step2.slope;
    result.fill_ratio_l = step3.fill_ratio_l;
    result.fill_ratio_w = step3.fill_ratio_w;
    result.fill_ratio_z = step3.fill_ratio_z;
    result.packing_density = step3.packing_density;
    result.confidence_score = step3.confidence_score;
    result.reasoning = format!(
        "3-step: h={:.2}m(step1) area={:.2}(step2) | {}",
        height, upper_area, step3.reasoning
    );

    // Calculate volume and tonnage from merged parameters
    calculate_volume_and_tonnage(&mut result);

    Ok(result)
}

/// Options for staged analysis
#[allow(dead_code)]
#[derive(Debug, Clone)]
pub struct StagedAnalysisOptions {
    /// Truck class for graded reference lookup (skip YOLO mode)
    pub truck_class: Option<TruckClass>,
    /// Ensemble count (number of inference iterations)
    pub ensemble_count: u32,
    /// Truck type pre-info (e.g., "4tダンプ")
    pub truck_type_hint: Option<String>,
    /// Material type pre-info (e.g., "As殻", "Co殻", "土砂")
    pub material_type: Option<String>,
    /// Karte JSON (known values; null means estimate)
    pub karte_json: Option<String>,
}

impl Default for StagedAnalysisOptions {
    fn default() -> Self {
        Self {
            truck_class: None,
            ensemble_count: 1,
            truck_type_hint: None,
            material_type: None,
            karte_json: None,
        }
    }
}

impl StagedAnalysisOptions {
    #[allow(dead_code)]
    pub fn with_truck_class(mut self, truck_class: TruckClass) -> Self {
        self.truck_class = Some(truck_class);
        self
    }

    #[allow(dead_code)]
    pub fn with_ensemble_count(mut self, count: u32) -> Self {
        self.ensemble_count = count.max(1);
        self
    }

    #[allow(dead_code)]
    pub fn with_truck_type_hint(mut self, truck_type: String) -> Self {
        self.truck_type_hint = Some(truck_type);
        self
    }

    #[allow(dead_code)]
    pub fn with_material_type(mut self, material_type: String) -> Self {
        self.material_type = Some(material_type);
        self
    }

    #[allow(dead_code)]
    pub fn with_karte_json(mut self, karte_json: String) -> Self {
        self.karte_json = Some(karte_json);
        self
    }
}

/// Staged analysis progress callback
pub type ProgressCallback = Box<dyn Fn(&str) + Send>;

/// Analyze image using staged approach with graded reference data
///
/// Stage 1: Initial inference to detect truck class (if truck_class not provided)
/// Stage 2+: Refined inference with graded historical data
pub fn analyze_image_staged(
    image_path: &Path,
    config: &AnalyzerConfig,
    options: &StagedAnalysisOptions,
    store: &Store,
    progress: Option<ProgressCallback>,
) -> Result<EstimationResult> {
    let notify = |msg: &str| {
        if let Some(ref cb) = progress {
            cb(msg);
        }
    };

    let mut graded_stock: Vec<GradedHistoryEntry> = Vec::new();
    let mut _detected_class = TruckClass::Unknown;
    let mut results: Vec<EstimationResult> = Vec::new();
    let target_count = options.ensemble_count.max(1) as usize;

    // If truck class is provided upfront, load graded data immediately
    if let Some(truck_class) = options.truck_class {
        _detected_class = truck_class;
        if _detected_class != TruckClass::Unknown {
            notify(&format!("{}クラスの実測データを取得中...", _detected_class.label()));
            graded_stock = store.select_stock_by_grade(_detected_class);
            if !graded_stock.is_empty() {
                notify(&format!("実測データ {}件を参照", graded_stock.len()));
            }
        }
    }

    for iteration in 0..target_count {
        notify(&format!("推論 {}/{} 実行中...", iteration + 1, target_count));

        // Build prompt based on available data
        let prompt = if let Some(karte_json) = &options.karte_json {
            build_karte_prompt(karte_json)
                .map_err(|e| Error::AnalysisFailed(format!("Invalid karte JSON: {}", e)))?
        } else if let (Some(truck_type), Some(material_type)) = (&options.truck_type_hint, &options.material_type) {
            // Use pre-filled prompt when both truck_type and material_type are provided
            build_estimation_prompt(truck_type, material_type)
        } else if !graded_stock.is_empty() {
            // Stage 2+: Use graded reference data
            let references: Vec<GradedReferenceItem> = graded_stock
                .iter()
                .map(|g| GradedReferenceItem {
                    grade_name: g.grade.label().to_string(),
                    actual_tonnage: g.entry.actual_tonnage.unwrap_or(0.0),
                    max_capacity: g.entry.max_capacity.unwrap_or(0.0),
                    load_ratio: g.load_ratio,
                    memo: g.entry.notes.clone(),
                })
                .collect();
            build_staged_analysis_prompt(None, &references)
        } else {
            // Stage 1: No reference data
            build_staged_analysis_prompt(None, &[])
        };

        // Configure AI options
        let mut ai_options = if let Some(ref model) = config.model {
            AnalyzeOptions::with_model(model)
        } else {
            AnalyzeOptions::default()
        };
        ai_options = ai_options.with_backend(config.backend).json().with_usage_mode(config.usage_mode);

        // Call AI
        let response = analyze(&prompt, &[image_path.to_path_buf()], ai_options)?;
        let result = parse_response(&response)?;

        // max_capacityが指定されていない場合は、graded_stockを取得せずにそのまま推論を続ける

        results.push(result);
    }

    if results.is_empty() {
        return Err(Error::AnalysisFailed("All inference attempts failed".to_string()));
    }

    // Merge results
    notify("結果を統合中...");
    Ok(merge_results(&results))
}

/// Analyze with staged approach (ensemble version)
#[allow(dead_code)]
pub fn analyze_image_staged_ensemble(
    image_path: &Path,
    config: &AnalyzerConfig,
    options: &StagedAnalysisOptions,
    store: &Store,
) -> Result<EstimationResult> {
    analyze_image_staged(image_path, config, options, store, None)
}

/// Parse AI response into EstimationResult
fn parse_response(response: &str) -> Result<EstimationResult> {
    // Try to extract JSON from response (may have markdown code blocks)
    let json_str = extract_json_from_response(response);

    // Parse JSON
    let mut result: EstimationResult = match serde_json::from_str(&json_str) {
        Ok(parsed) => parsed,
        Err(e) => {
            // Truncate response safely at char boundary
            let truncated: String = response.chars().take(500).collect();
            let mut fallback = EstimationResult::default();
            fallback.reasoning = format!(
                "[parse_error] {} | raw: {}",
                e, truncated
            );
            // Return minimal result to allow pipeline to proceed for testing
            return Ok(fallback);
        }
    };

    // Calculate volume and tonnage if not provided by AI (program-side calculation)
    if result.estimated_volume_m3 == 0.0 || result.estimated_tonnage == 0.0 {
        calculate_volume_and_tonnage(&mut result);
    }

    Ok(result)
}

/// Calculate volume and tonnage from estimated parameters using shared-core
fn calculate_volume_and_tonnage(result: &mut EstimationResult) {
    let height = result.height.unwrap_or(0.0);
    if height <= 0.0 {
        return;
    }

    let fill_ratio_w = result.fill_ratio_w.or(result.upper_area).unwrap_or(0.5);
    let fill_ratio_z = result.fill_ratio_z.or(result.fill_ratio).unwrap_or(0.85);

    let params = shared_core::CoreParams {
        fill_ratio_w,
        height,
        slope: result.slope.unwrap_or(0.0),
        fill_ratio_z,
        packing_density: result.packing_density.unwrap_or(0.80),
        material_type: result.material_type.clone(),
    };

    // Extract truck class (e.g., "4t" from "4tダンプ", "4tダンプ(土砂)" etc.)
    // shared-core defaults to 6.8m² (4t bed area) when class is None
    let truck_class = if result.truck_type.is_empty()
        || result.truck_type == "?"
        || result.truck_type == "？"
    {
        None
    } else {
        let cls = result.truck_type
            .split(|c: char| c == 'ダ' || c == '(' || c == '（')
            .next()
            .unwrap_or("")
            .trim()
            .to_string();
        if cls.is_empty() { None } else { Some(cls) }
    };

    // shared-core rounds: volume to 3 decimals, tonnage to 2 decimals
    let calc = shared_core::calculate_tonnage(&params, truck_class.as_deref());
    result.estimated_volume_m3 = calc.volume;
    result.estimated_tonnage = calc.tonnage;

    // Compute void_ratio for backward compatibility
    result.void_ratio = Some(1.0 - fill_ratio_z * params.packing_density);
}

/// Extract JSON from response (handles markdown code blocks)
pub fn extract_json_from_response(response: &str) -> String {
    let response = response.trim();

    // Check for markdown code block
    if response.starts_with("```json") {
        if let Some(end) = response.rfind("```") {
            let start = response.find('\n').unwrap_or(7) + 1;
            if start < end {
                return response[start..end].trim().to_string();
            }
        }
    }

    // Check for generic code block
    if response.starts_with("```") {
        if let Some(end) = response.rfind("```") {
            let start = response.find('\n').unwrap_or(3) + 1;
            if start < end {
                return response[start..end].trim().to_string();
            }
        }
    }

    // Try to find JSON object directly
    if let Some(start) = response.find('{') {
        if let Some(end) = response.rfind('}') {
            if start < end {
                return response[start..=end].to_string();
            }
        }
    }

    response.to_string()
}

/// Analyze multiple images (ensemble)
#[allow(dead_code)]
pub fn analyze_image_ensemble(
    image_path: &Path,
    config: &AnalyzerConfig,
    count: u32,
) -> Result<EstimationResult> {
    if count <= 1 {
        return analyze_image(image_path, config);
    }

    let mut results = Vec::new();

    for _ in 0..count {
        match analyze_image(image_path, config) {
            Ok(result) => results.push(result),
            Err(e) => eprintln!("Ensemble sample failed: {}", e),
        }
    }

    if results.is_empty() {
        return Err(Error::AnalysisFailed(
            "All ensemble samples failed".to_string(),
        ));
    }

    // Merge results
    Ok(merge_results(&results))
}

/// Merge multiple estimation results (ensemble voting)
fn merge_results(results: &[EstimationResult]) -> EstimationResult {
    if results.is_empty() {
        return EstimationResult::default();
    }

    if results.len() == 1 {
        return results[0].clone();
    }

    // Average numeric values
    let avg_volume: f64 = results.iter().map(|r| r.estimated_volume_m3).sum::<f64>()
        / results.len() as f64;
    let avg_tonnage: f64 =
        results.iter().map(|r| r.estimated_tonnage).sum::<f64>() / results.len() as f64;
    let avg_confidence: f64 =
        results.iter().map(|r| r.confidence_score).sum::<f64>() / results.len() as f64;

    // Use mode for categorical values
    let truck_type = mode_string(results.iter().map(|r| r.truck_type.clone()).collect());
    let material_type = mode_string(results.iter().map(|r| r.material_type.clone()).collect());

    // Use first result as base
    let mut merged = results[0].clone();
    merged.truck_type = truck_type;
    merged.material_type = material_type;
    merged.estimated_volume_m3 = avg_volume;
    merged.estimated_tonnage = avg_tonnage;
    merged.confidence_score = avg_confidence;
    merged.ensemble_count = Some(results.len() as u32);
    merged.reasoning = format!(
        "Ensemble average of {} samples. {}",
        results.len(),
        merged.reasoning
    );

    merged
}

/// Get mode (most common) of strings
fn mode_string(values: Vec<String>) -> String {
    use std::collections::HashMap;

    let mut counts: HashMap<String, usize> = HashMap::new();
    for v in values.iter() {
        *counts.entry(v.clone()).or_insert(0) += 1;
    }

    counts
        .into_iter()
        .max_by_key(|(_, count)| *count)
        .map(|(value, _)| value)
        .unwrap_or_default()
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_extract_json_markdown() {
        let response = "```json\n{\"test\": 123}\n```";
        assert_eq!(extract_json_from_response(response), "{\"test\": 123}");
    }

    #[test]
    fn test_extract_json_plain() {
        let response = "{\"test\": 123}";
        assert_eq!(extract_json_from_response(response), "{\"test\": 123}");
    }

    #[test]
    fn test_extract_json_with_text() {
        let response = "Here is the result: {\"test\": 123} end";
        assert_eq!(extract_json_from_response(response), "{\"test\": 123}");
    }
}
