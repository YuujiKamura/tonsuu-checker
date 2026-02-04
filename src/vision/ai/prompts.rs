//! AI prompts for image analysis - based on TonSuuChecker_local
//!
//! 段階的推論プロンプト:
//! - Step1: 車両・素材識別
//! - Step2: 高さ推定
//! - Step3: 空隙率推定
//! - Step4: 最終計算
//!
//! Note: Many prompts are prepared for future multi-step analysis.
//! Currently using simplified prompts, but step-by-step versions are maintained.

/// Core rules prompt (shared base for all prompts)
#[allow(dead_code)]
pub const CORE_RULES_PROMPT: &str = r#"あなたは建設廃棄物（ガラ）の重量推定を行うシステムです。

【最重要：創作・推測の禁止】
- 画像から確実に確認できる情報のみを使用すること
- 見えないもの、確認できないものについては「不明」「確認不可」と記載
- 憶測・仮説・想像に基づく記述は禁止

【回答形式】
- すべて日本語で回答
- 事実のみを簡潔に記述
- JSON形式で回答"#;

// ============================================================================
// 段階別プロンプト
// ============================================================================

/// 用語定義
#[allow(dead_code)]
pub const TERMINOLOGY_PROMPT: &str = r#"
【用語定義】
- 後板: 荷台後方の固定された板。赤色。高さ約30cm。
- 側板: 荷台側面の固定された板。後板と一体で箱型を形成。赤色。
- アオリ: 側板上部に取り付けられた可動式の板。青や白色。外側に倒れていることが多い。
- ヒンジ(連結部): 後板右上にある蝶番部分。荷台床面から約50cmの位置。
- 荷台床面: 積載物が載る面。
"#;

/// Step1: 高さ推定
#[allow(dead_code)]
pub const STEP1_HEIGHT_PROMPT: &str = r#"
【高さ推定】

4tダンプの荷台に積まれた廃棄物の高さを推定してください。

【スケール基準】
- 後板の高さ: 約30cm
- ヒンジ(連結部)までの高さ: 荷台床面から約50cm

【JSON形式】
{
  "height": number,
  "reasoning": "判定根拠"
}"#;

/// Step2: 上面積推定
#[allow(dead_code)]
pub const STEP2_UPPER_AREA_PROMPT: &str = r#"
【上面積推定】

積み上がっている山の頂部の面積（m²）を推定してください。
4tダンプ底面積は6.8m²(3.4m×2.0m)です。

【JSON形式】
{
  "upperArea": number,
  "reasoning": "判定根拠"
}"#;

/// Step3: 空隙率推定
#[allow(dead_code)]
pub const STEP3_VOID_RATIO_PROMPT: &str = r#"
【空隙率推定】

塊サイズから空隙率を推定してください。
後板の連結部(50cm)を基準に塊の大きさを判断してください。

【JSON形式】
{
  "voidRatio": number,
  "reasoning": "判定根拠"
}"#;


/// Volume estimation prompt (shared across all prompt functions)
/// 改善版: AIは推定のみ、計算はプログラム側で行う
pub const VOLUME_ESTIMATION_PROMPT: &str = r#"Output ONLY JSON: {"isTargetDetected":true,"truckType":"4tダンプ","licensePlate":null,"materialType":"???","upperArea":5.0,"height":0.4,"voidRatio":0.35,"confidenceScore":0.8,"reasoning":"???"}"#;

/// Build estimation prompt with pre-filled truck type and material type
/// AI fills in the null values from image analysis
pub fn build_estimation_prompt(truck_type: &str, material_type: &str) -> String {
    format!(
        r#"Fill null values from image. Output ONLY JSON: {{"isTargetDetected":true,"truckType":"{}","materialType":"{}","upperArea":null,"height":null,"voidRatio":null,"confidenceScore":null,"reasoning":null}}"#,
        truck_type, material_type
    )
}

/// Load grade definitions for prompt
#[allow(dead_code)]
pub const LOAD_GRADES_PROMPT: &str = r#"■ 積載等級（実測値 ÷ 最大積載量）
- 軽すぎ: 0〜80%
- 軽め: 80〜90%
- ちょうど: 90〜95%
- ギリOK: 95〜100%
- 積みすぎ: 100%超"#;

/// Registered vehicle info for prompt
#[allow(dead_code)]
pub struct RegisteredVehicleInfo {
    pub license_plate: String,
    pub name: String,
    pub max_capacity: f64,
}

/// Graded reference item for prompt building
#[allow(dead_code)]
pub struct GradedReferenceItem {
    pub grade_name: String,
    pub actual_tonnage: f64,
    pub max_capacity: f64,
    pub load_ratio: f64,
    pub memo: Option<String>,
}

// ============================================================================
// Prompt building functions - ALL use CORE_RULES_PROMPT + VOLUME_ESTIMATION_PROMPT
// ============================================================================

/// Build analysis prompt for a single image
pub fn build_analysis_prompt() -> String {
    VOLUME_ESTIMATION_PROMPT.to_string()
}

/// Build analysis prompt with max capacity instruction
#[allow(dead_code)]
pub fn build_analysis_prompt_with_capacity(max_capacity: Option<f64>) -> String {
    let capacity_instruction = if let Some(cap) = max_capacity {
        format!("【重要】この車両の最大積載量は{}トンです。\n\n", cap)
    } else {
        String::new()
    };

    format!(
        "{}{}\n{}\n\n画像を分析し、JSON形式で結果を返してください。",
        capacity_instruction, CORE_RULES_PROMPT, VOLUME_ESTIMATION_PROMPT
    )
}

/// Build analysis prompt with graded reference data (Stage 2+)
pub fn build_staged_analysis_prompt(
    _max_capacity: Option<f64>,
    _graded_references: &[GradedReferenceItem],
) -> String {
    // Simplified: just use VOLUME_ESTIMATION_PROMPT with fill-in-the-blanks JSON
    VOLUME_ESTIMATION_PROMPT.to_string()
}

/// Build batch analysis prompt
#[allow(dead_code)]
pub fn build_batch_prompt(image_count: usize) -> String {
    format!(
        "{}\n{}\n\n{}枚の画像を順番に分析し、各画像の結果をJSON配列で返してください。",
        CORE_RULES_PROMPT, VOLUME_ESTIMATION_PROMPT, image_count
    )
}

/// Build combined analysis prompt (plate crop + full image in one call)
#[allow(dead_code)]
pub fn build_combined_analysis_prompt(vehicles: &[RegisteredVehicleInfo]) -> String {
    let mut prompt = String::from(CORE_RULES_PROMPT);
    prompt.push_str("\n\n");

    prompt.push_str(r#"【画像の説明】
- 1枚目: ナンバープレートの拡大切り出し画像
- 2枚目: トラック全体の画像

【STEP 1: 車両識別】
A) ナンバープレート読み取り（1枚目）:
- 地名・分類番号3桁・ひらがな・一連番号4桁
- 「11-11」→1111、「11-22」→1122

B) 車体の特徴確認（2枚目）:
- 荷台の色・キャブの色・メーカーロゴ

"#);

    // Add registered vehicles
    if !vehicles.is_empty() {
        prompt.push_str("【登録車両リスト】\n");
        for v in vehicles {
            let last4: String = v.license_plate.chars().filter(|c| c.is_ascii_digit()).collect::<String>().chars().rev().take(4).collect::<String>().chars().rev().collect();
            prompt.push_str(&format!("- {} (一連番号: {}) → {} (最大{}t)\n",
                v.license_plate, last4, v.name, v.max_capacity));
        }
        prompt.push_str("\n★ リストに一致する場合のみlicensePlateを設定。一致しなければnull。\n\n");
    }

    prompt.push_str(VOLUME_ESTIMATION_PROMPT);
    prompt
}

/// Build combined analysis prompt with registered vehicle reference photos
#[allow(dead_code)]
pub fn build_combined_analysis_prompt_with_refs(
    vehicles: &[RegisteredVehicleInfo],
    vehicle_photos: &[(String, std::path::PathBuf)],
) -> String {
    let mut prompt = String::from(CORE_RULES_PROMPT);
    prompt.push_str("\n\n");

    prompt.push_str("【画像の説明】\n");
    prompt.push_str("- 1枚目: ナンバープレートの拡大切り出し画像（プレート読み取り用）\n");
    prompt.push_str("- 2枚目: 解析対象のトラック全体画像（積載量推定用）\n");

    // Add reference photo descriptions
    if !vehicle_photos.is_empty() {
        for (i, (plate, _)) in vehicle_photos.iter().enumerate() {
            prompt.push_str(&format!("- {}枚目: 登録車両「{}」の参照写真\n", i + 3, plate));
        }
        prompt.push_str("\n【重要】3枚目以降の参照写真と2枚目を見比べて、荷台の色・形状が一致する車両を特定してください。\n");
    }

    prompt.push_str(r#"
【STEP 1: 車両識別（ナンバー + 外観）】

A) ナンバープレート読み取り（1枚目）:
- 地名・分類番号3桁・ひらがな・一連番号4桁
- 「11-11」→1111、「11-22」→1122（ハイフン除去）
- 1111 ≠ 1122 ≠ 1133 ≠ 1177（全て別の番号）

B) 車体特徴照合（2枚目 vs 3枚目以降の参照写真）:
- 【最重要】荷台の色（赤い荷台 vs 白い荷台 vs 緑の荷台）
- キャブの色・車体のサイズ・形状

【照合ロジック】
1. まずナンバーを読み取る
2. 次に2枚目の荷台の色を確認
3. ナンバー読み取り結果と荷台色が矛盾したら → 荷台色を優先

"#);

    // Add registered vehicles
    if !vehicles.is_empty() {
        prompt.push_str("【登録車両リスト】\n");
        for (i, v) in vehicles.iter().enumerate() {
            let last4: String = v.license_plate.chars().filter(|c| c.is_ascii_digit()).collect::<String>().chars().rev().take(4).collect::<String>().chars().rev().collect();
            let photo_idx = vehicle_photos.iter().position(|(p, _)| p == &v.license_plate);
            let photo_ref = photo_idx.map(|idx| format!(" ← 参照写真{}枚目", idx + 3)).unwrap_or_default();
            prompt.push_str(&format!("{}. {} (一連番号: {}) → {} (最大{}t){}\n",
                i + 1, v.license_plate, last4, v.name, v.max_capacity, photo_ref));
        }
        prompt.push_str("\n");
    }

    prompt.push_str(VOLUME_ESTIMATION_PROMPT);
    prompt
}

// ============================================================================
// 段階別プロンプト取得
// ============================================================================

/// 段階別プロンプトを配列で定義（計算はプログラム側）
#[allow(dead_code)]
pub const STEP_PROMPTS: &[&str] = &[
    STEP1_HEIGHT_PROMPT,
    STEP2_UPPER_AREA_PROMPT,
    STEP3_VOID_RATIO_PROMPT,
];

/// 段階別プロンプトを取得 (0-indexed)
#[allow(dead_code)]
pub fn get_step_prompt(step: usize) -> Option<&'static str> {
    STEP_PROMPTS.get(step).copied()
}

// ============================================================================
// Legacy aliases (for backward compatibility)
// ============================================================================

/// Alias for build_analysis_prompt (backward compatibility)
#[allow(dead_code)]
pub fn build_analysis_prompt_with_vehicles(vehicles: &[RegisteredVehicleInfo]) -> String {
    build_combined_analysis_prompt(vehicles)
}

/// Legacy constant alias
#[allow(dead_code)]
pub const SYSTEM_PROMPT: &str = CORE_RULES_PROMPT;
