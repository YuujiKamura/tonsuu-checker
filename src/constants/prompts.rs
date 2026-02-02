//! AI prompts for image analysis - based on TonSuuChecker_local

#![allow(dead_code)]

/// Core rules prompt (shared base for all prompts)
pub const CORE_RULES_PROMPT: &str = r#"あなたは建設廃棄物（ガラ）の重量推定を行うシステムです。

【最重要：創作・推測の禁止】
- 画像から確実に確認できる情報のみを使用すること
- 見えないもの、確認できないものについては「不明」「確認不可」と記載
- 憶測・仮説・想像に基づく記述は禁止
- 与えられた計算式・密度・空隙率の数値をそのまま使用すること

【回答形式】
- すべて日本語で回答
- 事実のみを簡潔に記述

### 誤検出防止ルール
1. **対象確認 (isTargetDetected)**:
   - トラックの荷台に廃棄物が積載されている場合のみtrue
   - 空車、乗用車、風景などはfalse
2. **確信度 (confidenceScore)**: 0.0〜1.0、迷いがあれば0.7以下
3. **ナンバープレート照合**: リストにない場合はnull、創作禁止"#;

/// Volume estimation prompt (shared across all prompt functions)
pub const VOLUME_ESTIMATION_PROMPT: &str = r#"
【重量推定の計算式】
体積 = (upperArea + lowerArea) / 2 × height
重量 = 体積 × 密度 × (1 - voidRatio)

【固定値：底面積（lowerArea）】
- 4tダンプ: 底面3.4m×2.0m = 6.8m²（後板高0.34m）
- 増トン:   底面4.0m×2.2m = 8.8m²（後板高0.40m）

【AIが推定するパラメータ】

■ 上面積（upperArea）の推定方法:
  積載物の上面を俯瞰した面積（m²）
  - 平積み（すり切り）: 底面積とほぼ同じ（4t: 6.8m², 増トン: 8.8m²）
  - 山盛り: 底面積より小さい（山の頂上は狭くなる）
  - 軽い山盛り: 底面積の80〜90%程度（4t: 5.4〜6.1m²）
  - 高い山盛り: 底面積の60〜70%程度（4t: 4.1〜4.8m²）

■ 高さ（height）の推定方法:
  後板高をスケール基準として絶対値（m）で推定
  【スケール基準】
  - 4tダンプ後板高: 0.34m
  - 増トン後板高: 0.40m

  【高さの目安】※傾斜補正込みで控えめに推定すること
  - すり切り（後板ぴったり）: 4t=0.32m, 増トン=0.38m
  - 少し山盛り（後板の1.1倍相当）: 4t=0.34m, 増トン=0.40m
  - 山盛り（後板の1.2倍相当）: 4t=0.36m, 増トン=0.43m
  - 高い山盛り（後板の1.3倍相当）: 4t=0.38m, 増トン=0.45m
  ※積載物は傾斜していることが多いため、見た目より低めに推定する
  ※0.40m（4t）を超えることは稀


【重要：現実的な体積の制約】
- 4tダンプのすり切り体積は約2.3m³
- 山盛りでも2.5〜2.8m³が現実的な上限
- 3.0m³を超えることは極めて稀
- heightとupperAreaの推定時にこの制約を意識すること

【素材別密度】
- As殻/Co殻: 2.5 t/m³
- 土砂: 1.8 t/m³

【空隙率（voidRatio）の判定基準】
塊サイズで判定（荷台幅2m基準）
- 細かい（〜30cm）: 0.30
- 普通（30〜60cm）: 0.35
- 大きい（60cm〜）: 0.40
※遠近法補正: 荷台中央のガラは見た目より大きい→1段階上げる

【計算例】
例1: 4tダンプ、すり切り、As殻、普通サイズ
  upperArea = 6.8m²（平積みなので底面積と同じ）
  height = 0.34m
  体積 = (6.8 + 6.8) / 2 × 0.34 = 2.31m³
  重量 = 2.31 × 2.5 × (1 - 0.35) = 3.75t

例2: 4tダンプ、軽い山盛り、As殻、大きいサイズ
  upperArea = 5.5m²（山盛りで上面が狭い）
  height = 0.40m
  体積 = (5.5 + 6.8) / 2 × 0.40 = 2.46m³
  重量 = 2.46 × 2.5 × (1 - 0.40) = 3.69t

【中間計算値（必ず記入）】
- upperArea: 上面積（m²）
- height: 高さ（m）
- voidRatio: 空隙率（0.30〜0.40）

【回答形式】JSON:
{
  "isTargetDetected": boolean,
  "truckType": string,
  "licensePlate": string | null,
  "materialType": string,
  "upperArea": number,
  "height": number,
  "voidRatio": number,
  "estimatedVolumeM3": number,
  "estimatedTonnage": number,
  "confidenceScore": number,
  "reasoning": string
}"#;

/// Load grade definitions for prompt
pub const LOAD_GRADES_PROMPT: &str = r#"■ 積載等級（実測値 ÷ 最大積載量）
- 軽すぎ: 0〜80%
- 軽め: 80〜90%
- ちょうど: 90〜95%
- ギリOK: 95〜100%
- 積みすぎ: 100%超"#;

/// Registered vehicle info for prompt
pub struct RegisteredVehicleInfo {
    pub license_plate: String,
    pub name: String,
    pub max_capacity: f64,
}

/// Graded reference item for prompt building
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
    format!(
        "{}\n{}\n\n画像を分析し、JSON形式で結果を返してください。",
        CORE_RULES_PROMPT, VOLUME_ESTIMATION_PROMPT
    )
}

/// Build analysis prompt with max capacity instruction
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
    max_capacity: Option<f64>,
    graded_references: &[GradedReferenceItem],
) -> String {
    let mut prompt = String::new();

    // Max capacity instruction
    if let Some(cap) = max_capacity {
        prompt.push_str(&format!(
            "【重要】この車両の最大積載量は{}トンです。\n\n",
            cap
        ));
    }

    // Core rules
    prompt.push_str(CORE_RULES_PROMPT);
    prompt.push_str("\n");

    // Add graded reference data if available
    if !graded_references.is_empty() {
        prompt.push_str("\n");
        prompt.push_str(LOAD_GRADES_PROMPT);
        prompt.push_str("\n\n【実測データ】\n");

        for item in graded_references {
            let memo = item.memo.as_ref().map(|m| format!(" {}", m)).unwrap_or_default();
            prompt.push_str(&format!(
                "- 【{}】実測{:.1}t / 最大{:.1}t（{:.0}%）{}\n",
                item.grade_name, item.actual_tonnage, item.max_capacity, item.load_ratio, memo
            ));
        }
    }

    // Volume estimation (shared)
    prompt.push_str(VOLUME_ESTIMATION_PROMPT);
    prompt.push_str("\n\n画像を分析し、JSON形式で結果を返してください。");
    prompt
}

/// Build batch analysis prompt
pub fn build_batch_prompt(image_count: usize) -> String {
    format!(
        "{}\n{}\n\n{}枚の画像を順番に分析し、各画像の結果をJSON配列で返してください。",
        CORE_RULES_PROMPT, VOLUME_ESTIMATION_PROMPT, image_count
    )
}

/// Build combined analysis prompt (plate crop + full image in one call)
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
// Legacy aliases (for backward compatibility)
// ============================================================================

/// Alias for build_analysis_prompt (backward compatibility)
pub fn build_analysis_prompt_with_vehicles(vehicles: &[RegisteredVehicleInfo]) -> String {
    build_combined_analysis_prompt(vehicles)
}

/// Legacy constant alias
pub const SYSTEM_PROMPT: &str = CORE_RULES_PROMPT;
