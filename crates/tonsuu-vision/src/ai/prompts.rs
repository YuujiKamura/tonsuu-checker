//! AI prompts for image analysis - dump truck cargo weight estimation
//!
//! Prompts are designed to force the AI to visually analyze the image
//! rather than copy template values. Key techniques:
//! - No numeric example values in the JSON template (uses placeholders)
//! - Explicit visual observation criteria for each parameter
//! - Scale references (後板 height, ヒンジ position)
//!
//! Language convention:
//! - English for AI instructions (observe, estimate, output, etc.)
//! - Japanese for domain-specific terms (アスファルト殻, コンクリート殻, 土砂,
//!   後板, ヒンジ, ダンプ)

use std::sync::LazyLock;
use tonsuu_core::spec::SPEC;

// ============================================================================
// Multi-param prompt section from prompt-spec.json
// ============================================================================

/// Parsed multiParamPrompt section from prompt-spec.json
struct MultiParamPrompt {
    prompt_format: String,
    json_template: serde_json::Value,
    range_guide: String,
}

/// Parse the multiParamPrompt section from the raw embedded JSON.
/// This is needed because v2.1.0 moved prompt strings out of the top-level spec.
static MULTI_PARAM: LazyLock<MultiParamPrompt> = LazyLock::new(|| {
    let raw: serde_json::Value = serde_json::from_str(
        include_str!("../../../../../tonsuu-core/prompt-spec.json")
    ).expect("Failed to parse prompt-spec.json");

    let mp = &raw["multiParamPrompt"];
    MultiParamPrompt {
        prompt_format: mp["promptFormat"].as_str().unwrap_or("").to_string(),
        json_template: mp["jsonTemplate"].clone(),
        range_guide: mp["rangeGuide"].as_str().unwrap_or("").to_string(),
    }
});

// ============================================================================
// Truck bed dimension constants (meters) - loaded from prompt-spec.json
// ============================================================================

fn back_panel_height_m() -> f64 {
    SPEC.ranges.height.calibration.back_panel
}

fn hinge_height_m() -> f64 {
    SPEC.ranges.height.calibration.hinge
}

fn bed_area_m2() -> f64 {
    tonsuu_core::spec::default_bed_area()
}

// ============================================================================
// Estimation range constants
// ============================================================================

// ============================================================================
// Shared prompt fragments (used by multiple prompt builders)
// ============================================================================

/// Build the base JSON template structure with placeholders.
///
/// This shared function creates the core JSON structure that both
/// build_json_output_instruction and build_estimation_prompt use.
fn build_base_json_template(truck_type: &str, material_type: &str) -> serde_json::Value {
    let mut tmpl = MULTI_PARAM.json_template.clone();
    if let Some(obj) = tmpl.as_object_mut() {
        obj.insert("truckType".to_string(), serde_json::json!(truck_type));
        obj.insert("materialType".to_string(), serde_json::json!(material_type));
    }
    tmpl
}

/// Build the karte-mode observation guide.
///
/// Shorter than the full STEP 1/STEP 2 because the karte already provides
/// some values; only placeholder fields need estimation.
fn build_karte_observation_guide() -> String {
    format!(
        concat!(
            "\nAnalyze the cargo in the image. ",
            "Compare pile height to the 後板 tailgate top edge (~{back_panel:.1}m) ",
            "and ヒンジ (~{hinge:.1}m). ",
            "Estimate how much of the bed length is covered (fillRatioL) ",
            "and how much of the bed top is covered ",
            "(fillRatioW as fraction of {area:.1}m\u{00B2}). ",
            "Estimate how fully the pile reaches the ideal trapezoid shape (fillRatioZ) ",
            "and how tightly pieces are packed (packingDensity). ",
            "Replace every <estimate...> placeholder with your numeric estimate. ",
            "Write your visual observations in reasoning."
        ),
        back_panel = back_panel_height_m(),
        hinge = hinge_height_m(),
        area = bed_area_m2(),
    )
}

// ============================================================================
// Volume estimation prompt (the main prompt)
// ============================================================================

/// Build the full volume estimation prompt.
///
/// Compact format: JSON template on first line, ranges on second line.
/// Gemini ignores schemas in long prompts, so keep it minimal.
fn build_volume_estimation_prompt() -> String {
    let json_str = serde_json::to_string(&MULTI_PARAM.json_template)
        .unwrap_or_else(|_| "{}".to_string());
    MULTI_PARAM.prompt_format
        .replace("{jsonTemplate}", &json_str)
        .replace("{rangeGuide}", &MULTI_PARAM.range_guide)
}

/// Volume estimation prompt - the core prompt used by all analysis paths.
///
/// Design: Forces AI to observe image details by requiring visual reasoning
/// before numeric estimation. JSON template uses string placeholders to
/// prevent the AI from copying example numbers.
///
/// NOTE: This is a `static` string built once via `std::sync::LazyLock`.
/// All dimension constants (後板, ヒンジ, bed size) are injected from the
/// module-level constants so they are defined in one place.
pub static VOLUME_ESTIMATION_PROMPT: LazyLock<String> =
    LazyLock::new(build_volume_estimation_prompt);

/// Graded reference item for prompt building (used by staged analysis)
pub struct GradedReferenceItem {
    pub grade_name: String,
    pub actual_tonnage: f64,
    pub max_capacity: f64,
    pub load_ratio: f64,
    pub memo: Option<String>,
}

// ============================================================================
// Prompt builders
// ============================================================================

/// Build analysis prompt for a single image (no pre-info)
pub fn build_analysis_prompt() -> String {
    VOLUME_ESTIMATION_PROMPT.clone()
}

/// Build estimation prompt with pre-filled truck type and material type.
///
/// When the operator already knows the truck and material, we inject those
/// so the AI only needs to estimate the geometric parameters from the image.
pub fn build_estimation_prompt(truck_type: &str, material_type: &str) -> String {
    let json_template = build_base_json_template(truck_type, material_type);
    let json_str = serde_json::to_string(&json_template)
        .unwrap_or_else(|_| "{}".to_string());
    MULTI_PARAM.prompt_format
        .replace("{jsonTemplate}", &json_str)
        .replace("{rangeGuide}", &MULTI_PARAM.range_guide)
}

/// Build estimation prompt with Karte JSON (partially pre-filled values).
///
/// Non-null values from the karte are locked in; null fields must be estimated
/// by the AI from the image. The prompt injects observation instructions and
/// uses string placeholders for null fields to prevent value copying.
pub fn build_karte_prompt(karte_json: &str) -> Result<String, String> {
    let mut parsed: serde_json::Value = serde_json::from_str(karte_json)
        .map_err(|e| format!("Failed to parse karte JSON: {}", e))?;

    let obj = parsed.as_object_mut()
        .ok_or_else(|| "Karte JSON is not an object".to_string())?;

    // Replace null or missing fields with 0 (AI must estimate from image)
    let numeric_fields = [
        "height",
        "fillRatioL",
        "fillRatioW",
        "fillRatioZ",
        "packingDensity",
        "confidenceScore",
    ];

    for field in &numeric_fields {
        let needs_zero = match obj.get(*field) {
            None => true,
            Some(v) => v.is_null(),
        };
        if needs_zero {
            obj.insert(field.to_string(), serde_json::json!(0));
        }
    }

    // Ensure reasoning placeholder exists
    let needs_reasoning = match obj.get("reasoning") {
        None => true,
        Some(v) => v.is_null(),
    };
    if needs_reasoning {
        obj.insert(
            "reasoning".to_string(),
            serde_json::json!("describe what you observe"),
        );
    }

    // Ensure isTargetDetected is a valid boolean
    let needs_detected = match obj.get("isTargetDetected") {
        None => true,
        Some(v) => !v.is_boolean(),
    };
    if needs_detected {
        obj.insert("isTargetDetected".to_string(), serde_json::json!(true));
    }

    // Ensure licensePlate key exists (null is fine)
    if !obj.contains_key("licensePlate") {
        obj.insert("licensePlate".to_string(), serde_json::Value::Null);
    }

    let guide = build_karte_observation_guide();

    let serialized = serde_json::to_string(&parsed)
        .map_err(|e| format!("Failed to serialize modified karte JSON: {}", e))?;

    Ok(format!(
        "Output ONLY JSON with this exact schema (replace all 0 with your estimates):\n{}{}",
        serialized, guide
    ))
}

/// Build analysis prompt with staged graded reference data.
///
/// When graded historical data is available, it is appended as calibration
/// context so the AI can compare the current load against past known weights.
///
/// # Why graded references are handled carefully (TODO: staged-v2)
///
/// The graded reference integration is intentionally minimal. Showing the AI
/// reference images or detailed load-ratio distributions was deferred because
/// early experiments revealed a critical **anchoring problem**: when given
/// historical tonnage values or reference photos, the AI tends to pattern-match
/// against the closest reference example instead of independently observing
/// the current image. This leads to estimation convergence toward reference
/// values rather than true visual analysis.
///
/// For example, if shown "Grade A: 3.5t with pile at hinge height," the AI
/// will estimate ~3.5t for any pile near the hinge, regardless of void ratio
/// or bed coverage differences. The current design provides only summary
/// statistics to calibrate scale intuition without creating strong anchors.
///
/// Future work (staged-v2): Explore prompt techniques that preserve reference
/// utility while preventing anchoring (e.g., showing reference ranges instead
/// of exact values, requiring explicit comparison justification, or using
/// contrastive examples).
pub fn build_staged_analysis_prompt(
    max_capacity: Option<f64>,
    graded_references: &[GradedReferenceItem],
) -> String {
    let base = build_volume_estimation_prompt();

    // If no references are available, return the base prompt as-is.
    if graded_references.is_empty() && max_capacity.is_none() {
        return base;
    }

    let mut prompt = base;

    // Append max capacity context if provided
    if let Some(cap) = max_capacity {
        prompt.push_str(&format!(
            "\n\nAdditional context: This truck has a maximum legal capacity of {:.1}t. \
             Use this only as a sanity-check upper bound, not as a target.",
            cap
        ));
    }

    // Append graded reference summary if available
    if !graded_references.is_empty() {
        prompt.push_str("\n\nHistorical reference data (for calibration only - \
                         do NOT copy these values, observe the image independently):\n");
        for item in graded_references {
            let memo_suffix = item
                .memo
                .as_deref()
                .filter(|m| !m.is_empty())
                .map(|m| format!(" ({})", m))
                .unwrap_or_default();
            prompt.push_str(&format!(
                "- Grade {}: actual {:.1}t / max {:.1}t (load ratio {:.0}%){}\n",
                item.grade_name,
                item.actual_tonnage,
                item.max_capacity,
                item.load_ratio * 100.0,
                memo_suffix,
            ));
        }
        prompt.push_str(
            "Use these references to calibrate your scale sense, \
             but base your estimates on what you observe in the image.",
        );
    }

    prompt
}


// ============================================================================
// Tests
// ============================================================================

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_constants_from_prompt_spec() {
        // All values should be read from prompt-spec.json (SSOT) — no hardcoded expectations
        let bp = back_panel_height_m();
        let hi = hinge_height_m();
        let area = bed_area_m2();
        // Sanity: values should be positive and within reasonable physical bounds
        assert!(bp > 0.0 && bp < 1.0, "back_panel {bp} out of range");
        assert!(hi > 0.0 && hi < 1.0, "hinge {hi} out of range");
        assert!(area > 1.0 && area < 20.0, "bed_area {area} out of range");
        // Hinge should be higher than back panel
        assert!(hi > bp, "hinge ({hi}) should be higher than back_panel ({bp})");
    }

    #[test]
    fn test_volume_estimation_prompt_contains_dimensions() {
        let prompt = &*VOLUME_ESTIMATION_PROMPT;
        // Scale reference constants from prompt-spec.json must appear in the prompt text
        let bp_str = format!("後板(テールゲート上縁)={:.2}m", back_panel_height_m());
        let hi_str = format!("ヒンジ金具={:.2}m", hinge_height_m());
        assert!(prompt.contains(&bp_str), "missing 後板上端 height: expected {bp_str}");
        assert!(prompt.contains(&hi_str), "missing ヒンジ height: expected {hi_str}");
        // Height should request 0.05m step estimation
        assert!(prompt.contains("0.05m刻み"), "missing 0.05m step instruction");
    }

    #[test]
    fn test_volume_estimation_prompt_uses_japanese_domain_terms() {
        let prompt = &*VOLUME_ESTIMATION_PROMPT;
        assert!(prompt.contains("後板"), "missing 後板");
        assert!(prompt.contains("ヒンジ"), "missing ヒンジ");
    }

    #[test]
    fn test_volume_estimation_prompt_uses_english_instructions() {
        let prompt = &*VOLUME_ESTIMATION_PROMPT;
        assert!(prompt.contains("Output ONLY JSON"), "missing JSON instruction");
        assert!(prompt.contains("Adjust each value"), "missing range guide");
    }

    #[test]
    fn test_build_analysis_prompt_returns_base() {
        let prompt = build_analysis_prompt();
        assert_eq!(prompt, *VOLUME_ESTIMATION_PROMPT);
    }

    #[test]
    fn test_build_estimation_prompt_injects_truck_and_material() {
        let prompt = build_estimation_prompt("4tダンプ", "アスファルト殻");
        assert!(prompt.contains("4tダンプ"));
        assert!(prompt.contains("アスファルト殻"));
        // Contains range references from prompt-spec.json
        let bp_str = format!("後板(テールゲート上縁)={:.2}m", back_panel_height_m());
        let hi_str = format!("ヒンジ金具={:.2}m", hinge_height_m());
        assert!(prompt.contains(&bp_str), "missing 後板上端 height in estimation prompt");
        assert!(prompt.contains(&hi_str), "missing ヒンジ height in estimation prompt");
    }

    #[test]
    fn test_build_estimation_prompt_no_duplication_drift() {
        // Both prompts should use the same rangeGuide from SPEC
        let base = &*VOLUME_ESTIMATION_PROMPT;
        let est = build_estimation_prompt("X", "Y");
        // Both must contain the key range terms from rangeGuide
        for keyword in &["height(", "fillRatioL(", "fillRatioW(", "fillRatioZ(", "packingDensity("] {
            assert!(base.contains(keyword), "base missing {keyword}");
            assert!(est.contains(keyword), "est missing {keyword}");
        }
    }

    #[test]
    fn test_build_karte_prompt_replaces_nulls() {
        let karte = r#"{"truckType":"4t","materialType":"As殻","height":null,"fillRatioL":null,"fillRatioW":null,"fillRatioZ":null,"packingDensity":null}"#;
        let prompt = build_karte_prompt(karte).expect("should succeed with valid JSON");
        // Null fields should be replaced with 0
        assert!(prompt.contains("\"height\":0"));
        assert!(prompt.contains("\"fillRatioL\":0"));
        assert!(prompt.contains("\"fillRatioW\":0"));
        assert!(prompt.contains("\"fillRatioZ\":0"));
        assert!(prompt.contains("\"packingDensity\":0"));
        // Pre-filled values should be preserved
        assert!(prompt.contains("\"truckType\":\"4t\""));
        assert!(prompt.contains("\"materialType\":\"As殻\""));
    }

    #[test]
    fn test_build_karte_prompt_preserves_existing_values() {
        let karte = r#"{"truckType":"4t","materialType":"As殻","height":0.3,"fillRatioL":0.7,"fillRatioW":0.6,"fillRatioZ":0.85,"packingDensity":0.8}"#;
        let prompt = build_karte_prompt(karte).expect("should succeed with valid JSON");
        // Should NOT replace non-null values with 0
        assert!(prompt.contains("\"fillRatioW\":0.6"));
    }

    #[test]
    fn test_build_karte_prompt_invalid_json_returns_err() {
        let result = build_karte_prompt("not json at all");
        assert!(result.is_err());
        assert!(result.unwrap_err().contains("Failed to parse karte JSON"));
    }

    #[test]
    fn test_build_karte_prompt_non_object_returns_err() {
        let result = build_karte_prompt("[1, 2, 3]");
        assert!(result.is_err());
        assert!(result.unwrap_err().contains("Karte JSON is not an object"));
    }

    #[test]
    fn test_build_karte_prompt_contains_guide() {
        let karte = r#"{"truckType":"4t"}"#;
        let prompt = build_karte_prompt(karte).expect("should succeed with valid JSON");
        assert!(prompt.contains("Analyze the cargo"));
        assert!(prompt.contains("後板"));
        assert!(prompt.contains("ヒンジ"));
    }

    #[test]
    fn test_build_staged_no_references() {
        let prompt = build_staged_analysis_prompt(None, &[]);
        assert_eq!(prompt, *VOLUME_ESTIMATION_PROMPT);
    }

    #[test]
    fn test_build_staged_with_max_capacity() {
        let prompt = build_staged_analysis_prompt(Some(10.0), &[]);
        assert!(prompt.contains("10.0t"));
        assert!(prompt.contains("sanity-check"));
    }

    #[test]
    fn test_build_staged_with_references() {
        let refs = vec![
            GradedReferenceItem {
                grade_name: "A".to_string(),
                actual_tonnage: 3.5,
                max_capacity: 4.0,
                load_ratio: 0.875,
                memo: Some("full load".to_string()),
            },
            GradedReferenceItem {
                grade_name: "C".to_string(),
                actual_tonnage: 1.5,
                max_capacity: 4.0,
                load_ratio: 0.375,
                memo: None,
            },
        ];
        let prompt = build_staged_analysis_prompt(Some(4.0), &refs);
        assert!(prompt.contains("Grade A: actual 3.5t"));
        assert!(prompt.contains("Grade C: actual 1.5t"));
        assert!(prompt.contains("(full load)"));
        assert!(prompt.contains("do NOT copy these values"));
        assert!(prompt.contains("4.0t"));
    }


}
