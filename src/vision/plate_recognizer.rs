//! Local license plate detection using YOLO (detection only, no OCR).

use crate::config::Config;
use crate::error::Result;
use crate::vision::extract_json_from_response;
use serde::Deserialize;
use std::path::{Path, PathBuf};
use std::process::Command;

#[allow(dead_code)]
#[derive(Debug, Deserialize)]
pub struct PlateDetectionResult {
    pub detected: bool,
    pub confidence: Option<f32>,
    pub bbox: Option<Vec<i32>>,
    pub crop_path: Option<String>,
    pub elapsed_ms: Option<u32>,
    pub error: Option<String>,
}

/// Detect license plate using YOLO and return cropped image path.
/// Returns Ok(Some(crop_path)) on success, Ok(None) on failure or disabled.
#[allow(dead_code)]
pub fn detect_plate_yolo(
    image_path: &Path,
    config: &Config,
    verbose: bool,
) -> Result<Option<(PathBuf, f32)>> {
    if !config.plate_local_enabled {
        return Ok(None);
    }

    let cmd_str = match config.plate_local_command.as_ref() {
        Some(cmd) if !cmd.trim().is_empty() => cmd,
        _ => {
            if verbose {
                eprintln!("plate_local is enabled but plate_local_command is not set.");
            }
            return Ok(None);
        }
    };

    let mut parts = match shell_words::split(cmd_str) {
        Ok(parts) if !parts.is_empty() => parts,
        _ => {
            if verbose {
                eprintln!("plate_local_command is invalid: {}", cmd_str);
            }
            return Ok(None);
        }
    };

    // Create temp file for cropped plate
    let temp_dir = std::env::temp_dir();
    let crop_path = temp_dir.join(format!("plate_crop_{}.jpg", std::process::id()));

    let program = parts.remove(0);
    let mut cmd = Command::new(&program);
    cmd.args(&parts);
    cmd.arg("--image");
    cmd.arg(image_path);
    cmd.arg("--min-conf");
    cmd.arg(format!("{}", config.plate_local_min_conf));
    cmd.arg("--output-crop");
    cmd.arg(&crop_path);

    if verbose {
        eprintln!("Running: {} {:?} --image {:?} --output-crop {:?}",
            program, parts, image_path, crop_path);
    }

    let output = match cmd.output() {
        Ok(output) => output,
        Err(err) => {
            if verbose {
                eprintln!("plate_local execution failed: {}", err);
            }
            return Ok(None);
        }
    };

    if !output.status.success() {
        if verbose {
            let stderr = String::from_utf8_lossy(&output.stderr);
            eprintln!("plate_local error: {}", stderr.trim());
        }
        return Ok(None);
    }

    let stdout = String::from_utf8_lossy(&output.stdout);
    if stdout.trim().is_empty() {
        return Ok(None);
    }

    let json_str = extract_json_from_response(stdout.as_ref());
    let parsed: PlateDetectionResult = match serde_json::from_str(&json_str) {
        Ok(parsed) => parsed,
        Err(err) => {
            if verbose {
                eprintln!("plate_local JSON parse error: {} - response: {}", err, json_str);
            }
            return Ok(None);
        }
    };

    if !parsed.detected {
        if verbose {
            eprintln!("YOLO: No plate detected");
        }
        return Ok(None);
    }

    let confidence = parsed.confidence.unwrap_or(0.0);

    if verbose {
        eprintln!(
            "YOLO: Plate detected (conf {:.1}%, {}ms)",
            confidence * 100.0,
            parsed.elapsed_ms.unwrap_or(0)
        );
    }

    // Check if crop file exists
    if !crop_path.exists() {
        if verbose {
            eprintln!("YOLO: Crop file not created");
        }
        return Ok(None);
    }

    Ok(Some((crop_path, confidence)))
}

/// Clean up temporary crop file
#[allow(dead_code)]
pub fn cleanup_crop(crop_path: &Path) {
    let _ = std::fs::remove_file(crop_path);
}
