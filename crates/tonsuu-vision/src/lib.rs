//! Vision module - AI-powered image analysis for tonnage estimation

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
pub use ai::backend_impl::CliAiBackend;
pub use cache::Cache;
#[allow(unused_imports)]
pub use volume_estimator::analyze_shaken;

use tonsuu_types::{Error, Result};
use tonsuu_store::{GradedHistoryEntry, Store};
use tonsuu_types::{EstimationResult, TruckClass};
use cli_ai_analyzer::{analyze, AnalyzeOptions, Backend, UsageMode};
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

/// Analyze a single image and return estimation result.
///
/// **Deprecated**: Prefer `analyze_image_box_overlay` for higher accuracy.
/// This legacy single-prompt path is kept for the GUI and ground-truth tests.
#[deprecated(note = "Use analyze_image_box_overlay for higher accuracy")]
pub fn analyze_image(image_path: &Path, config: &AnalyzerConfig) -> Result<EstimationResult> {
    let prompt = build_analysis_prompt();

    let mut options = if let Some(ref model) = config.model {
        AnalyzeOptions::with_model(model)
    } else {
        AnalyzeOptions::default()
    };

    options = options.with_backend(config.backend).json().with_usage_mode(config.usage_mode);

    let response = analyze(&prompt, &[image_path.to_path_buf()], options)?;

    parse_response(&response)
}

/// Analyze a single image using the box-overlay pipeline (geometry + fill two-stage).
///
/// This is the recommended analysis path, producing more accurate results than
/// the legacy multi-param single-prompt approach.
pub fn analyze_image_box_overlay(
    image_path: &Path,
    config: &AnalyzerConfig,
    truck_class: &str,
    material_type: &str,
    ensemble_count: usize,
    progress: Option<ProgressCallback>,
) -> Result<EstimationResult> {
    let notify = |msg: &str| {
        if let Some(ref cb) = progress {
            cb(msg);
        }
    };

    notify("Box-overlay解析を準備中...");

    let mut options = if let Some(ref model) = config.model {
        AnalyzeOptions::with_model(model)
    } else {
        AnalyzeOptions::default()
    };
    options = options.with_backend(config.backend).json().with_usage_mode(config.usage_mode);

    let backend = CliAiBackend {
        options,
        image_paths: vec![image_path.to_path_buf()],
    };

    let pipeline_config = tonsuu_core::BoxOverlayConfig {
        truck_class: truck_class.to_string(),
        material_type: material_type.to_string(),
        ensemble_count,
    };

    notify("AI推論実行中...");

    let result = tonsuu_core::analyze_box_overlay(&backend, &[], &pipeline_config)
        .map_err(|e| Error::AnalysisFailed(e.to_string()))?;

    notify("結果を変換中...");

    // Convert pipeline result to EstimationResult for backward compatibility
    let mut estimation = EstimationResult::default();
    estimation.truck_type = truck_class.to_string();
    estimation.material_type = material_type.to_string();
    estimation.height = Some(result.height_m);
    estimation.fill_ratio_l = Some(result.fill_ratio_l);
    estimation.fill_ratio_w = Some(result.fill_ratio_w);
    estimation.packing_density = Some(result.packing_density);
    estimation.estimated_volume_m3 = result.volume;
    estimation.estimated_tonnage = result.tonnage;
    let success_rate = result.geometry_runs.iter().filter(|r| r.parsed.is_some()).count() as f64
        / result.geometry_runs.len().max(1) as f64;
    estimation.confidence_score = 0.6 + 0.3 * success_rate; // 0.6~0.9 based on ensemble success
    estimation.reasoning = result.reasoning;
    estimation.ensemble_count = Some(ensemble_count as u32);

    Ok(estimation)
}

/// Options for staged analysis
#[allow(dead_code)]
#[derive(Debug, Clone)]
pub struct StagedAnalysisOptions {
    pub truck_class: Option<TruckClass>,
    pub ensemble_count: u32,
    pub truck_type_hint: Option<String>,
    pub material_type: Option<String>,
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

        let prompt = if let Some(karte_json) = &options.karte_json {
            build_karte_prompt(karte_json)
                .map_err(|e| Error::AnalysisFailed(format!("Invalid karte JSON: {}", e)))?
        } else if let (Some(truck_type), Some(material_type)) = (&options.truck_type_hint, &options.material_type) {
            build_estimation_prompt(truck_type, material_type)
        } else if !graded_stock.is_empty() {
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
            build_staged_analysis_prompt(None, &[])
        };

        let mut ai_options = if let Some(ref model) = config.model {
            AnalyzeOptions::with_model(model)
        } else {
            AnalyzeOptions::default()
        };
        ai_options = ai_options.with_backend(config.backend).json().with_usage_mode(config.usage_mode);

        let response = analyze(&prompt, &[image_path.to_path_buf()], ai_options)?;
        let result = parse_response(&response)?;

        results.push(result);
    }

    if results.is_empty() {
        return Err(Error::AnalysisFailed("All inference attempts failed".to_string()));
    }

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
    let json_str = extract_json_from_response(response);

    let mut result: EstimationResult = match serde_json::from_str(&json_str) {
        Ok(parsed) => parsed,
        Err(e) => {
            let truncated: String = response.chars().take(500).collect();
            let mut fallback = EstimationResult::default();
            fallback.reasoning = format!(
                "[parse_error] {} | raw: {}",
                e, truncated
            );
            return Ok(fallback);
        }
    };

    if result.estimated_volume_m3 == 0.0 || result.estimated_tonnage == 0.0 {
        calculate_volume_and_tonnage(&mut result);
    }

    Ok(result)
}

/// Calculate volume and tonnage from estimated parameters using shared-core.
///
/// Maps multi-param AI output (fillRatioL/W/Z) to box-overlay CoreParams.
/// fillRatioZ from the old multi-param strategy is not used in the new formula;
/// taper_ratio defaults to 0.85 since the multi-param prompt doesn't ask for it.
fn calculate_volume_and_tonnage(result: &mut EstimationResult) {
    let height = result.height.unwrap_or(0.0);
    if height <= 0.0 {
        return;
    }

    let params = tonsuu_core::CoreParams {
        height,
        fill_ratio_l: result.fill_ratio_l.unwrap_or(0.8),
        fill_ratio_w: result.fill_ratio_w.unwrap_or(0.5),
        taper_ratio: 0.85,  // multi-param doesn't estimate taper; use reasonable default
        packing_density: result.packing_density.unwrap_or(0.80),
        material_type: result.material_type.clone(),
    };

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

    let calc = tonsuu_core::calculate_tonnage(&params, truck_class.as_deref());
    result.estimated_volume_m3 = calc.volume;
    result.estimated_tonnage = calc.tonnage;
}

/// Extract JSON from response (handles markdown code blocks)
pub fn extract_json_from_response(response: &str) -> String {
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


/// Merge multiple estimation results (ensemble voting)
fn merge_results(results: &[EstimationResult]) -> EstimationResult {
    if results.is_empty() {
        return EstimationResult::default();
    }

    if results.len() == 1 {
        return results[0].clone();
    }

    let avg_volume: f64 = results.iter().map(|r| r.estimated_volume_m3).sum::<f64>()
        / results.len() as f64;
    let avg_tonnage: f64 =
        results.iter().map(|r| r.estimated_tonnage).sum::<f64>() / results.len() as f64;
    let avg_confidence: f64 =
        results.iter().map(|r| r.confidence_score).sum::<f64>() / results.len() as f64;

    let truck_type = mode_string(results.iter().map(|r| r.truck_type.clone()).collect());
    let material_type = mode_string(results.iter().map(|r| r.material_type.clone()).collect());

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
