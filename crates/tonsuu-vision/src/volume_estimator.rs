//! Vehicle registration certificate (shaken) analyzer and volume estimation

use tonsuu_types::{Error, Result};
use crate::AnalyzerConfig;
use cli_ai_analyzer::{analyze, AnalyzeOptions};
use serde::{Deserialize, Serialize};
use std::path::Path;

/// Result of 車検証 (vehicle registration certificate) analysis
#[allow(dead_code)]
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct ShakenResult {
    /// Vehicle name (車名), e.g., "日野 プロフィア"
    pub vehicle_name: String,
    /// Maximum payload capacity in tonnes (最大積載量)
    pub max_capacity: f64,
    /// Registration number (登録番号), optional
    #[serde(default)]
    pub registration_number: Option<String>,
}

/// Build the prompt for 車検証 analysis
#[allow(dead_code)]
fn build_shaken_prompt() -> String {
    r#"あなたは車検証（自動車検査証）を読み取る専門家です。
提供された車検証の画像から以下の情報を正確に読み取ってください。

## 読み取る項目

1. **車名と型式**: 車検証に記載されている「車名」欄を読み取ってください。
   - 例: "日野 プロフィア", "いすゞ ギガ", "三菱ふそう スーパーグレート", "UDトラックス クオン"

2. **最大積載量**: 車検証に記載されている「最大積載量」を読み取り、**トン単位**で返してください。
   - 車検証にはkg単位で記載されていることが多いので、その場合は1000で割ってトンに変換してください
   - 例: 11,500kg → 11.5 (トン)
   - 例: 4,000kg → 4.0 (トン)

3. **登録番号**: 車検証に記載されている「登録番号」（ナンバープレートの番号）を読み取ってください。
   - 例: "品川 100 あ 12-34"
   - 読み取れない場合はnullを返してください

## 出力形式

以下のJSON形式で出力してください：

```json
{
  "vehicleName": "車名（メーカー名と車種名）",
  "maxCapacity": 最大積載量（トン単位の数値）,
  "registrationNumber": "登録番号またはnull"
}
```

## 注意事項

- 車検証が不鮮明な場合でも、可能な限り読み取りを試みてください
- 数値は必ず数値型で返してください（文字列にしないでください）
- 最大積載量は必ずトン単位に変換してください
- 車名が読み取れない場合は「不明」と返してください
- 最大積載量が読み取れない場合は0.0を返してください"#
        .to_string()
}

/// Extract JSON from AI response (handles markdown code blocks)
#[allow(dead_code)]
fn extract_json(response: &str) -> String {
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

/// Analyze a 車検証 (vehicle registration certificate) image
#[allow(dead_code)]
pub fn analyze_shaken(image_path: &Path, config: &AnalyzerConfig) -> Result<ShakenResult> {
    let prompt = build_shaken_prompt();

    let mut options = if let Some(ref model) = config.model {
        AnalyzeOptions::with_model(model)
    } else {
        AnalyzeOptions::default()
    };

    options = options.with_backend(config.backend).json();

    let response = analyze(&prompt, &[image_path.to_path_buf()], options)?;

    parse_shaken_response(&response)
}

/// Parse AI response into ShakenResult
#[allow(dead_code)]
fn parse_shaken_response(response: &str) -> Result<ShakenResult> {
    let json_str = extract_json(response);

    let result: ShakenResult = serde_json::from_str(&json_str).map_err(|e| {
        let truncated: String = response.chars().take(500).collect();
        Error::AnalysisFailed(format!(
            "Failed to parse 車検証 analysis response: {}. Response: {}",
            e, truncated
        ))
    })?;

    Ok(result)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_parse_shaken_response_valid() {
        let response = r#"```json
{
  "vehicleName": "日野 プロフィア",
  "maxCapacity": 11.5,
  "registrationNumber": "品川 100 あ 12-34"
}
```"#;

        let result = parse_shaken_response(response).unwrap();
        assert_eq!(result.vehicle_name, "日野 プロフィア");
        assert!((result.max_capacity - 11.5).abs() < 0.001);
        assert_eq!(
            result.registration_number,
            Some("品川 100 あ 12-34".to_string())
        );
    }

    #[test]
    fn test_parse_shaken_response_null_registration() {
        let response = r#"{
  "vehicleName": "いすゞ ギガ",
  "maxCapacity": 10.0,
  "registrationNumber": null
}"#;

        let result = parse_shaken_response(response).unwrap();
        assert_eq!(result.vehicle_name, "いすゞ ギガ");
        assert!((result.max_capacity - 10.0).abs() < 0.001);
        assert!(result.registration_number.is_none());
    }
}
