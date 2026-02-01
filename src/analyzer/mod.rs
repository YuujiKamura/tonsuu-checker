//! AI-powered image analysis using cli-ai-analyzer

pub mod cache;

use crate::constants::prompts::build_analysis_prompt;
use crate::error::{Error, Result};
use crate::types::EstimationResult;
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

/// Parse AI response into EstimationResult
fn parse_response(response: &str) -> Result<EstimationResult> {
    // Try to extract JSON from response (may have markdown code blocks)
    let json_str = extract_json(response);

    // Parse JSON
    let result: EstimationResult = serde_json::from_str(&json_str).map_err(|e| {
        Error::AnalysisFailed(format!(
            "Failed to parse AI response: {}. Response: {}",
            e,
            &response[..response.len().min(500)]
        ))
    })?;

    Ok(result)
}

/// Extract JSON from response (handles markdown code blocks)
fn extract_json(response: &str) -> String {
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
        assert_eq!(extract_json(response), "{\"test\": 123}");
    }

    #[test]
    fn test_extract_json_plain() {
        let response = "{\"test\": 123}";
        assert_eq!(extract_json(response), "{\"test\": 123}");
    }

    #[test]
    fn test_extract_json_with_text() {
        let response = "Here is the result: {\"test\": 123} end";
        assert_eq!(extract_json(response), "{\"test\": 123}");
    }
}
