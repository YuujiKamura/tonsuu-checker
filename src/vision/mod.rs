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
pub use volume_estimator::analyze_shaken;

use crate::error::{Error, Result};
use crate::store::{GradedHistoryEntry, Store};
use crate::types::{EstimationResult, TruckClass};
use cli_ai_analyzer::{analyze, AnalyzeOptions, Backend};
use std::path::Path;

/// Analyzer configuration
#[derive(Debug, Clone)]
pub struct AnalyzerConfig {
    pub backend: Backend,
    pub model: Option<String>,
}

impl Default for AnalyzerConfig {
    fn default() -> Self {
        Self {
            backend: Backend::Gemini,
            model: None,
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

    options = options.with_backend(config.backend).json();

    // Call AI analyzer
    let response = analyze(&prompt, &[image_path.to_path_buf()], options)?;

    // Parse response
    parse_response(&response)
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
        ai_options = ai_options.with_backend(config.backend).json();

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

/// Calculate volume and tonnage from estimated parameters
/// Formula: 体積 = (upperArea + lowerArea) / 2 × height
///          重量 = 体積 × 密度 × (1 - voidRatio)
fn calculate_volume_and_tonnage(result: &mut EstimationResult) {
    const LOWER_AREA: f64 = 6.8; // 4tダンプ底面積 (m²)

    // Get density from material type
    let density = match result.material_type.as_str() {
        s if s.contains("土砂") => 1.8,
        _ => 2.5, // As殻/Co殻 default
    };

    // Get parameters with defaults
    let upper_area = result.upper_area.unwrap_or(LOWER_AREA);
    let height = result.height.unwrap_or(0.0);
    let void_ratio = result.void_ratio.unwrap_or(0.35);

    // Calculate
    if height > 0.0 {
        let volume = (upper_area + LOWER_AREA) / 2.0 * height;
        let tonnage = volume * density * (1.0 - void_ratio);

        result.estimated_volume_m3 = (volume * 100.0).round() / 100.0; // Round to 2 decimals
        result.estimated_tonnage = (tonnage * 100.0).round() / 100.0;
    }
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
