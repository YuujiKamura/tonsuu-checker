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

/// Typed representation of prompt-spec.json
#[derive(serde::Deserialize)]
struct PromptSpec {
    ranges: Ranges,
    calculation: Calculation,
}

#[derive(serde::Deserialize)]
struct Ranges {
    #[serde(rename = "upperArea")]
    upper_area: MinMax,
    height: HeightRange,
    slope: MinMax,
}

#[derive(serde::Deserialize)]
struct MinMax {
    min: f64,
    max: f64,
}

#[derive(serde::Deserialize)]
struct HeightRange {
    min: f64,
    max: f64,
    calibration: HeightCalibration,
}

#[derive(serde::Deserialize)]
struct HeightCalibration {
    #[serde(rename = "後板")]
    back_panel: f64,
    #[serde(rename = "ヒンジ")]
    hinge: f64,
}

#[derive(serde::Deserialize)]
struct Calculation {
    #[serde(rename = "defaultBedAreaM2")]
    default_bed_area_m2: f64,
}

/// Parsed prompt-spec.json (shared specification)
static PROMPT_SPEC: LazyLock<PromptSpec> = LazyLock::new(|| {
    let raw = include_str!("../../../prompt-spec.json");
    serde_json::from_str(raw).expect("Failed to parse prompt-spec.json")
});

// ============================================================================
// Truck bed dimension constants (meters) - loaded from prompt-spec.json
// ============================================================================

fn back_panel_height_m() -> f64 {
    PROMPT_SPEC.ranges.height.calibration.back_panel
}

fn hinge_height_m() -> f64 {
    PROMPT_SPEC.ranges.height.calibration.hinge
}

fn bed_area_m2() -> f64 {
    PROMPT_SPEC.calculation.default_bed_area_m2
}

// ============================================================================
// Estimation range constants
// ============================================================================

fn upper_area_range() -> (f64, f64) {
    (PROMPT_SPEC.ranges.upper_area.min, PROMPT_SPEC.ranges.upper_area.max)
}

fn height_range() -> (f64, f64) {
    (PROMPT_SPEC.ranges.height.min, PROMPT_SPEC.ranges.height.max)
}

fn slope_range() -> (f64, f64) {
    (PROMPT_SPEC.ranges.slope.min, PROMPT_SPEC.ranges.slope.max)
}

/// Fill ratio range (0.7~1.0): how well the pile silhouette fills the frustum shape
/// Packing density range (0.7~0.9): how tightly debris pieces are packed together

// ============================================================================
// JSON field name constants
// ============================================================================

const KEY_UPPER_AREA: &str = "upperArea";
const KEY_HEIGHT: &str = "height";
const KEY_SLOPE: &str = "slope";
const KEY_FILL_RATIO: &str = "fillRatio";
const KEY_PACKING_DENSITY: &str = "packingDensity";
const KEY_FILL_RATIO_L: &str = "fillRatioL";
const KEY_FILL_RATIO_W: &str = "fillRatioW";
const KEY_FILL_RATIO_Z: &str = "fillRatioZ";
const KEY_CONFIDENCE_SCORE: &str = "confidenceScore";
const KEY_REASONING: &str = "reasoning";
const KEY_IS_TARGET_DETECTED: &str = "isTargetDetected";
const KEY_LICENSE_PLATE: &str = "licensePlate";
const KEY_TRUCK_TYPE: &str = "truckType";
const KEY_MATERIAL_TYPE: &str = "materialType";

// ============================================================================
// Shared prompt fragments (used by multiple prompt builders)
// ============================================================================

/// Build the base JSON template structure with placeholders.
///
/// This shared function creates the core JSON structure that both
/// build_json_output_instruction and build_estimation_prompt use.
fn build_base_json_template(truck_type: &str, material_type: &str) -> serde_json::Value {
    serde_json::json!({
        KEY_IS_TARGET_DETECTED: true,
        KEY_TRUCK_TYPE: truck_type,
        KEY_LICENSE_PLATE: null,
        KEY_MATERIAL_TYPE: material_type,
        KEY_UPPER_AREA: 0,
        KEY_HEIGHT: 0,
        KEY_SLOPE: 0,
        KEY_PACKING_DENSITY: 0,
        KEY_FILL_RATIO_L: 0,
        KEY_FILL_RATIO_W: 0,
        KEY_FILL_RATIO_Z: 0,
        KEY_CONFIDENCE_SCORE: 0,
        KEY_REASONING: "describe what you see"
    })
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
            "Estimate how much of the bed the pile top covers ",
            "(upperArea as fraction of {area:.1}m\u{00B2}). ",
            "Judge how well the pile fills the bed shape (fillRatio) and how tightly pieces are packed (packingDensity). ",
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

/// Build the shared range guide string for numeric parameters.
///
/// Provides concrete visual criteria for each parameter so the AI
/// can distinguish fillRatio from packingDensity independently.
///
/// Height calibration: The prompt forces the AI to judge pile height
/// relative to two visible landmarks (後板 top = 0.3m, ヒンジ = 0.5m)
/// and estimate in 0.05m steps for finer discrimination.
fn build_range_guide() -> String {
    format!(
        concat!(
            "upperArea({ua_min:.1}~{ua_max:.1}) ",
            "height({h_min:.2}~{h_max:.2}, 0.05m刻みで推定せよ。",
            "後板(テールゲート上縁)={bp:.2}m, ヒンジ金具={hi:.2}m。",
            "荷山の最高点がどちらの目印の何cm上/下かを見て数値化せよ) ",
            "slope({s_min:.1}~{s_max:.1}, 荷山の前後方向の高低差m: 手前が低ければ正値) ",
            "fillRatioL(0.7~1.0, 長さ方向の充填率: 荷台の前後方向にどこまで積まれているか) ",
            "fillRatioW(0.7~1.0, 幅方向の充填率: 荷台の左右方向にどこまで積まれているか) ",
            "fillRatioZ(0.7~1.0, 高さ方向の充填率: 錐台形状に対して山がどこまで埋まっているか) ",
            "packingDensity(0.7~0.9, ガラの詰まり具合) ",
            "※fillRatioL/W/Zはそれぞれ独立して推定すること"
        ),
        ua_min = upper_area_range().0,
        ua_max = upper_area_range().1,
        h_min = height_range().0,
        h_max = height_range().1,
        bp = back_panel_height_m(),
        hi = hinge_height_m(),
        s_min = slope_range().0,
        s_max = slope_range().1,
    )
}

/// Build the full volume estimation prompt.
///
/// Compact format: JSON template on first line, ranges on second line.
/// Gemini ignores schemas in long prompts, so keep it minimal.
fn build_volume_estimation_prompt() -> String {
    let json_template = build_base_json_template("?", "?");
    let json_str = serde_json::to_string(&json_template)
        .unwrap_or_else(|_| "{}".to_string());
    let range_guide = build_range_guide();

    format!(
        "Output ONLY JSON: {} Adjust each value based on the image: {}",
        json_str, range_guide
    )
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
    let range_guide = build_range_guide();

    format!(
        "Output ONLY JSON: {} Adjust each value based on the image: {}",
        json_str, range_guide
    )
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
        KEY_UPPER_AREA,
        KEY_HEIGHT,
        KEY_SLOPE,
        KEY_FILL_RATIO,
        KEY_PACKING_DENSITY,
        KEY_CONFIDENCE_SCORE,
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
    let needs_reasoning = match obj.get(KEY_REASONING) {
        None => true,
        Some(v) => v.is_null(),
    };
    if needs_reasoning {
        obj.insert(
            KEY_REASONING.to_string(),
            serde_json::json!("describe what you observe"),
        );
    }

    // Ensure isTargetDetected is a valid boolean
    let needs_detected = match obj.get(KEY_IS_TARGET_DETECTED) {
        None => true,
        Some(v) => !v.is_boolean(),
    };
    if needs_detected {
        obj.insert(KEY_IS_TARGET_DETECTED.to_string(), serde_json::json!(true));
    }

    // Ensure licensePlate key exists (null is fine)
    if !obj.contains_key(KEY_LICENSE_PLATE) {
        obj.insert(KEY_LICENSE_PLATE.to_string(), serde_json::Value::Null);
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
// Step-specific prompt builders (for multi-step analysis)
// ============================================================================

/// Step 1 for 2-step: Estimate height + identify truck/material.
/// Fewer fields = more AI attention on height accuracy.
pub fn build_step1_height_prompt() -> String {
    format!(
        concat!(
            "Output ONLY JSON: ",
            "{{\"truckType\":\"?\",\"materialType\":\"?\",\"height\":0,\"reasoning\":\"describe what you see\"}} ",
            "Estimate the cargo pile height in 0.05m steps. ",
            "後板(テールゲート上縁)={bp:.2}m, ヒンジ金具={hi:.2}m。",
            "荷山の最高点がどちらの目印の何cm上/下かを見て数値化せよ"
        ),
        bp = back_panel_height_m(),
        hi = hinge_height_m(),
    )
}

/// Step 2 for 2-step: Estimate remaining parameters with height locked in.
pub fn build_step2_rest_prompt(height: f64, truck_type: &str, material_type: &str) -> String {
    format!(
        concat!(
            "Output ONLY JSON: ",
            "{{\"upperArea\":0,\"slope\":0,",
            "\"fillRatioL\":0,\"fillRatioW\":0,\"fillRatioZ\":0,",
            "\"packingDensity\":0,\"confidenceScore\":0,",
            "\"reasoning\":\"describe what you see\"}} ",
            "The cargo height is {height:.2}m, truck is \"{truck_type}\", material is \"{material_type}\". ",
            "Estimate remaining: ",
            "upperArea({ua_min:.1}~{ua_max:.1}) ",
            "slope({s_min:.1}~{s_max:.1}, 荷山の前後高低差m) ",
            "fillRatioL(0.7~1.0, 長さ方向) ",
            "fillRatioW(0.7~1.0, 幅方向) ",
            "fillRatioZ(0.7~1.0, 高さ方向) ",
            "packingDensity(0.7~0.9, ガラの詰まり具合) ",
            "※fillRatioL/W/Zはそれぞれ独立して推定すること"
        ),
        height = height,
        truck_type = truck_type,
        material_type = material_type,
        ua_min = upper_area_range().0,
        ua_max = upper_area_range().1,
        s_min = slope_range().0,
        s_max = slope_range().1,
    )
}

/// Step 1 for 3-step: Height ONLY (maximum attention).
pub fn build_step1_height_only_prompt() -> String {
    format!(
        concat!(
            "Output ONLY JSON: ",
            "{{\"height\":0,\"reasoning\":\"describe what you see\"}} ",
            "Estimate ONLY the cargo pile height in 0.05m steps. ",
            "後板(テールゲート上縁)={bp:.2}m, ヒンジ金具={hi:.2}m。",
            "荷山の最高点がどちらの目印の何cm上/下かを見て数値化せよ。",
            "Focus exclusively on height measurement."
        ),
        bp = back_panel_height_m(),
        hi = hinge_height_m(),
    )
}

/// Step 2 for 3-step: Area + slope (given height).
pub fn build_step2_area_prompt(height: f64) -> String {
    format!(
        concat!(
            "Output ONLY JSON: ",
            "{{\"truckType\":\"?\",\"materialType\":\"?\",",
            "\"upperArea\":0,\"slope\":0,",
            "\"reasoning\":\"describe what you see\"}} ",
            "The cargo height is {height:.2}m. ",
            "Estimate: upperArea({ua_min:.1}~{ua_max:.1}, fraction of {area:.1}m² bed) ",
            "slope({s_min:.1}~{s_max:.1}, 荷山の前後高低差m)"
        ),
        height = height,
        ua_min = upper_area_range().0,
        ua_max = upper_area_range().1,
        area = bed_area_m2(),
        s_min = slope_range().0,
        s_max = slope_range().1,
    )
}

/// Step 3 for 3-step: Fill ratios + packing (given height + area).
pub fn build_step3_fill_prompt(height: f64, upper_area: f64) -> String {
    format!(
        concat!(
            "Output ONLY JSON: ",
            "{{\"fillRatioL\":0,\"fillRatioW\":0,\"fillRatioZ\":0,",
            "\"packingDensity\":0,\"confidenceScore\":0,",
            "\"reasoning\":\"describe what you see\"}} ",
            "The cargo height is {height:.2}m, upperArea is {ua:.2}. ",
            "Estimate: ",
            "fillRatioL(0.7~1.0, 長さ方向) ",
            "fillRatioW(0.7~1.0, 幅方向) ",
            "fillRatioZ(0.7~1.0, 高さ方向) ",
            "packingDensity(0.7~0.9, ガラの詰まり具合) ",
            "※fillRatioL/W/Zはそれぞれ独立して推定すること"
        ),
        height = height,
        ua = upper_area,
    )
}

// ============================================================================
// Vehicle-related prompt builders (used by combined plate+cargo analysis)
// ============================================================================

/// Registered vehicle info for prompt
pub struct RegisteredVehicleInfo {
    pub license_plate: String,
    pub name: String,
    pub max_capacity: f64,
}

/// Extract the last 4 digits from a license plate string.
fn extract_last4_digits(plate: &str) -> String {
    let digits: Vec<char> = plate.chars().filter(|c| c.is_ascii_digit()).collect();
    let start = digits.len().saturating_sub(4);
    digits[start..].iter().collect()
}

/// Build combined analysis prompt (plate crop + full image in one call)
#[allow(dead_code)]
pub fn build_combined_analysis_prompt(vehicles: &[RegisteredVehicleInfo]) -> String {
    let mut prompt = String::new();

    prompt.push_str(
        "You have two images:\n\
         - Image 1: Cropped license plate\n\
         - Image 2: Full truck photo\n\
         \n\
         STEP 1 - Read the license plate from Image 1:\n\
         - Read: region, classification (3 digits), hiragana, serial (4 digits)\n\
         \n\
         STEP 2 - Identify the truck from Image 2:\n\
         - Note bed color, cab color, manufacturer\n\n",
    );

    if !vehicles.is_empty() {
        prompt.push_str("Registered vehicles:\n");
        for v in vehicles {
            let last4 = extract_last4_digits(&v.license_plate);
            prompt.push_str(&format!(
                "- {} (serial: {}) = {} (max {}t)\n",
                v.license_plate, last4, v.name, v.max_capacity
            ));
        }
        prompt.push_str(
            "\nSet licensePlate only if plate matches a registered vehicle. Otherwise null.\n\n",
        );
    }

    prompt.push_str(&VOLUME_ESTIMATION_PROMPT);
    prompt
}

/// Build combined analysis prompt with registered vehicle reference photos
///
/// # Parameter note: `vehicle_photos` PathBuf usage
///
/// The `vehicle_photos` parameter is `&[(String, PathBuf)]` where:
/// - `String` = license plate number, used in the prompt to label reference images
/// - `PathBuf` = file path to the reference photo, used by the **caller** to load
///   the image file before sending to the AI vision model
///
/// The `PathBuf` itself does NOT appear in the prompt text. The caller is
/// responsible for reading the image files and passing them to the AI model
/// in the correct order (plate crop, target photo, then reference photos 1..N).
/// This function only generates the text prompt that describes the image layout.
#[allow(dead_code)]
pub fn build_combined_analysis_prompt_with_refs(
    vehicles: &[RegisteredVehicleInfo],
    vehicle_photos: &[(String, std::path::PathBuf)],
) -> String {
    let mut prompt = String::new();

    prompt.push_str("Image layout:\n");
    prompt.push_str("- Image 1: License plate crop\n");
    prompt.push_str("- Image 2: Target truck full photo\n");

    if !vehicle_photos.is_empty() {
        for (i, (plate, _)) in vehicle_photos.iter().enumerate() {
            prompt.push_str(&format!(
                "- Image {}: Reference photo for \"{}\"\n",
                i + 3,
                plate
            ));
        }
        prompt.push_str(
            "\nCompare Image 2 bed color/shape with reference photos to identify the truck.\n",
        );
    }

    prompt.push_str(
        "\nSTEP 1 - Read license plate (Image 1):\n\
         - region, classification, hiragana, serial (4 digits, remove hyphens)\n\
         \n\
         STEP 2 - Match truck (Image 2 vs reference photos):\n\
         - Compare bed color (red vs white vs green)\n\
         - If plate reading contradicts bed color, trust bed color\n\n",
    );

    if !vehicles.is_empty() {
        prompt.push_str("Registered vehicles:\n");
        for (i, v) in vehicles.iter().enumerate() {
            let last4 = extract_last4_digits(&v.license_plate);
            let photo_idx = vehicle_photos.iter().position(|(p, _)| p == &v.license_plate);
            let photo_ref = photo_idx
                .map(|idx| format!(" (see Image {})", idx + 3))
                .unwrap_or_default();
            prompt.push_str(&format!(
                "{}. {} (serial: {}) = {} (max {}t){}\n",
                i + 1,
                v.license_plate,
                last4,
                v.name,
                v.max_capacity,
                photo_ref
            ));
        }
        prompt.push('\n');
    }

    prompt.push_str(&VOLUME_ESTIMATION_PROMPT);
    prompt
}

/// Legacy constant alias
#[allow(dead_code)]
pub const SYSTEM_PROMPT: &str =
    "You are a construction debris weight estimation system. Analyze dump truck cargo images.";

// ============================================================================
// Tests
// ============================================================================

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_constants_consistent() {
        // bed_area_m2 should match prompt-spec.json defaultBedAreaM2
        let expected = 6.8;
        assert!(
            (bed_area_m2() - expected).abs() < f64::EPSILON,
            "bed_area_m2() ({}) != {} (from prompt-spec.json)",
            bed_area_m2(),
            expected
        );
        // Calibration constants should match
        assert!((back_panel_height_m() - 0.3).abs() < f64::EPSILON);
        assert!((hinge_height_m() - 0.5).abs() < f64::EPSILON);
    }

    #[test]
    fn test_volume_estimation_prompt_contains_dimensions() {
        let prompt = &*VOLUME_ESTIMATION_PROMPT;
        // Scale reference constants must appear in the prompt text
        assert!(prompt.contains("後板(テールゲート上縁)=0.30m"), "missing 後板上端 height");
        assert!(prompt.contains("ヒンジ金具=0.50m"), "missing ヒンジ height");
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
        // Contains range references
        assert!(prompt.contains("後板(テールゲート上縁)=0.30m"), "missing 後板上端 height in estimation prompt");
        assert!(prompt.contains("ヒンジ金具=0.50m"), "missing ヒンジ height in estimation prompt");
    }

    #[test]
    fn test_build_estimation_prompt_no_duplication_drift() {
        // Both prompts should use the same range constants
        let base = &*VOLUME_ESTIMATION_PROMPT;
        let est = build_estimation_prompt("X", "Y");
        assert!(base.contains("upperArea(0.2~0.6)"));
        assert!(est.contains("upperArea(0.2~0.6)"));
        assert!(base.contains("fillRatioL(0.7~1.0"));
        assert!(base.contains("fillRatioW(0.7~1.0"));
        assert!(base.contains("fillRatioZ(0.7~1.0"));
        assert!(est.contains("fillRatioL(0.7~1.0"));
        assert!(est.contains("fillRatioW(0.7~1.0"));
        assert!(est.contains("fillRatioZ(0.7~1.0"));
    }

    #[test]
    fn test_build_karte_prompt_replaces_nulls() {
        let karte = r#"{"truckType":"4t","materialType":"As殻","upperArea":null,"height":null,"slope":null,"fillRatio":null,"packingDensity":null}"#;
        let prompt = build_karte_prompt(karte).expect("should succeed with valid JSON");
        // Null fields should be replaced with 0
        assert!(prompt.contains("\"upperArea\":0"));
        assert!(prompt.contains("\"height\":0"));
        assert!(prompt.contains("\"slope\":0"));
        assert!(prompt.contains("\"fillRatio\":0"));
        assert!(prompt.contains("\"packingDensity\":0"));
        // Pre-filled values should be preserved
        assert!(prompt.contains("\"truckType\":\"4t\""));
        assert!(prompt.contains("\"materialType\":\"As殻\""));
    }

    #[test]
    fn test_build_karte_prompt_preserves_existing_values() {
        let karte = r#"{"truckType":"4t","materialType":"As殻","upperArea":0.45,"height":0.3,"slope":0.1,"fillRatio":0.85,"packingDensity":0.8}"#;
        let prompt = build_karte_prompt(karte).expect("should succeed with valid JSON");
        // Should NOT replace non-null values with 0
        assert!(prompt.contains("0.45") || prompt.contains("\"upperArea\":0.45"));
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

    #[test]
    fn test_extract_last4_digits() {
        assert_eq!(extract_last4_digits("品川 500 あ 1234"), "1234");
        assert_eq!(extract_last4_digits("12-34"), "1234");
        assert_eq!(extract_last4_digits("99"), "99");
        assert_eq!(extract_last4_digits("no digits"), "");
    }

    #[test]
    fn test_build_combined_analysis_prompt_no_vehicles() {
        let prompt = build_combined_analysis_prompt(&[]);
        assert!(prompt.contains("Image 1: Cropped license plate"));
        assert!(prompt.contains("Image 2: Full truck photo"));
        // Should contain the base prompt
        assert!(prompt.contains("Output ONLY JSON"));
    }

    #[test]
    fn test_build_combined_analysis_prompt_with_vehicles() {
        let vehicles = vec![RegisteredVehicleInfo {
            license_plate: "品川500あ1234".to_string(),
            name: "Test Truck".to_string(),
            max_capacity: 4.0,
        }];
        let prompt = build_combined_analysis_prompt(&vehicles);
        assert!(prompt.contains("品川500あ1234"));
        assert!(prompt.contains("Test Truck"));
        assert!(prompt.contains("max 4t"));
        assert!(prompt.contains("Registered vehicles"));
    }

    #[test]
    fn test_step1_height_prompt_contains_references() {
        let prompt = build_step1_height_prompt();
        assert!(prompt.contains("後板(テールゲート上縁)=0.30m"));
        assert!(prompt.contains("ヒンジ金具=0.50m"));
        assert!(prompt.contains("0.05m"));
        assert!(prompt.contains("height"));
        // Should NOT contain fillRatio fields
        assert!(!prompt.contains("fillRatio"));
        assert!(!prompt.contains("upperArea"));
    }

    #[test]
    fn test_step2_rest_prompt_locks_height() {
        let prompt = build_step2_rest_prompt(0.45, "4t", "As殻");
        assert!(prompt.contains("0.45m"));
        assert!(prompt.contains("4t"));
        assert!(prompt.contains("As殻"));
        assert!(prompt.contains("fillRatioL"));
        assert!(prompt.contains("fillRatioW"));
        assert!(prompt.contains("fillRatioZ"));
        // Should NOT contain height estimation range
        assert!(!prompt.contains("後板"));
    }

    #[test]
    fn test_step1_height_only_fewer_fields() {
        let prompt = build_step1_height_only_prompt();
        assert!(prompt.contains("height"));
        assert!(prompt.contains("reasoning"));
        // Must NOT contain other estimation fields
        assert!(!prompt.contains("upperArea"));
        assert!(!prompt.contains("fillRatio"));
        assert!(!prompt.contains("packingDensity"));
        assert!(!prompt.contains("truckType"));
    }

    #[test]
    fn test_3step_prompts_chain() {
        let s2 = build_step2_area_prompt(0.40);
        assert!(s2.contains("0.40m"));
        assert!(s2.contains("upperArea"));

        let s3 = build_step3_fill_prompt(0.40, 0.5);
        assert!(s3.contains("0.40m"));
        assert!(s3.contains("0.50"));
        assert!(s3.contains("fillRatioL"));
    }
}
