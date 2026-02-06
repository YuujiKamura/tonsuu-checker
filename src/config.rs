//! Configuration management for tonsuu-checker
//!
//! Config stored at: ~/.config/tonsuu-checker/config.json
//! Truck and material specs stored at: config/trucks.toml and config/materials.toml

use crate::cli::OutputFormat;
use crate::domain::{MaterialSpec, TruckSpec};
use crate::error::{ConfigError, Result};
use serde::{Deserialize, Serialize};
use std::collections::HashMap;
use std::path::PathBuf;
use std::sync::OnceLock;

/// Truck entry in TOML config
#[derive(Debug, Clone, Deserialize)]
pub struct TruckConfigEntry {
    pub id: String,
    pub name: String,
    pub max_capacity: f64,
    pub bed_length: f64,
    pub bed_width: f64,
    pub bed_height: f64,
    pub level_volume: f64,
    pub heap_volume: f64,
    #[serde(default)]
    pub aliases: Vec<String>,
}

/// Trucks config file structure
#[derive(Debug, Clone, Deserialize)]
pub struct TrucksConfig {
    pub trucks: Vec<TruckConfigEntry>,
}

/// Material entry in TOML config
/// Note: Prepared for material specification loading. Currently unused.
#[allow(dead_code)]
#[derive(Debug, Clone, Deserialize)]
pub struct MaterialConfigEntry {
    pub id: String,
    pub name: String,
    pub density: f64,
    pub void_ratio: f64,
}

/// Materials config file structure
/// Note: Prepared for material specification loading. Currently unused.
#[allow(dead_code)]
#[derive(Debug, Clone, Deserialize)]
pub struct MaterialsConfig {
    pub materials: Vec<MaterialConfigEntry>,
}

/// Loaded truck specs with aliases
pub struct LoadedTruckSpecs {
    pub specs: HashMap<String, TruckSpec>,
    pub aliases: HashMap<String, String>,
}

/// Loaded material specs
/// Note: Prepared for material specification loading. Currently unused.
#[allow(dead_code)]
pub struct LoadedMaterialSpecs {
    pub specs: HashMap<String, MaterialSpec>,
}

// Static storage for loaded specs (stores Result to handle errors)
static LOADED_TRUCK_SPECS: OnceLock<std::result::Result<LoadedTruckSpecs, String>> = OnceLock::new();
#[allow(dead_code)]
static LOADED_MATERIAL_SPECS: OnceLock<std::result::Result<LoadedMaterialSpecs, String>> = OnceLock::new();

/// Get the config directory path relative to the executable or project root
fn get_config_dir() -> PathBuf {
    // Try to find config relative to executable first
    if let Ok(exe_path) = std::env::current_exe() {
        if let Some(exe_dir) = exe_path.parent() {
            let config_dir = exe_dir.join("config");
            if config_dir.exists() {
                return config_dir;
            }
            // Also check parent directory (for target/debug layout)
            if let Some(parent) = exe_dir.parent() {
                let config_dir = parent.join("config");
                if config_dir.exists() {
                    return config_dir;
                }
                // Check two levels up (for target/debug/tonsuu-checker layout)
                if let Some(grandparent) = parent.parent() {
                    let config_dir = grandparent.join("config");
                    if config_dir.exists() {
                        return config_dir;
                    }
                }
            }
        }
    }

    // Fall back to current working directory
    std::env::current_dir()
        .unwrap_or_else(|_| PathBuf::from("."))
        .join("config")
}

/// Internal function to load truck specs
fn load_truck_specs_internal() -> std::result::Result<LoadedTruckSpecs, String> {
    let config_path = get_config_dir().join("trucks.toml");
    let content = std::fs::read_to_string(&config_path).map_err(|e| {
        format!(
            "Failed to read trucks.toml from {}: {}",
            config_path.display(),
            e
        )
    })?;

    let config: TrucksConfig = toml::from_str(&content)
        .map_err(|e| format!("Failed to parse trucks.toml: {}", e))?;

    let mut specs = HashMap::new();
    let mut aliases = HashMap::new();

    for entry in config.trucks {
        let spec = TruckSpec {
            name: entry.name,
            max_capacity: entry.max_capacity,
            bed_length: entry.bed_length,
            bed_width: entry.bed_width,
            bed_height: entry.bed_height,
            level_volume: entry.level_volume,
            heap_volume: entry.heap_volume,
        };
        specs.insert(entry.id.clone(), spec);

        for alias in entry.aliases {
            aliases.insert(alias, entry.id.clone());
        }
    }

    Ok(LoadedTruckSpecs { specs, aliases })
}

/// Load truck specs from TOML config file
pub fn load_truck_specs() -> Result<&'static LoadedTruckSpecs> {
    let result = LOADED_TRUCK_SPECS.get_or_init(load_truck_specs_internal);
    match result {
        Ok(specs) => Ok(specs),
        Err(e) => Err(ConfigError::ParseError(e.clone()).into()),
    }
}

/// Internal function to load material specs
/// Note: Prepared for material specification loading. Currently unused.
#[allow(dead_code)]
fn load_material_specs_internal() -> std::result::Result<LoadedMaterialSpecs, String> {
    let config_path = get_config_dir().join("materials.toml");
    let content = std::fs::read_to_string(&config_path).map_err(|e| {
        format!(
            "Failed to read materials.toml from {}: {}",
            config_path.display(),
            e
        )
    })?;

    let config: MaterialsConfig = toml::from_str(&content)
        .map_err(|e| format!("Failed to parse materials.toml: {}", e))?;

    let mut specs = HashMap::new();

    for entry in config.materials {
        let spec = MaterialSpec {
            name: entry.name,
            density: entry.density,
            void_ratio: entry.void_ratio,
        };
        specs.insert(entry.id, spec);
    }

    Ok(LoadedMaterialSpecs { specs })
}

/// Load material specs from TOML config file
/// Note: Prepared for material specification loading. Currently unused.
#[allow(dead_code)]
pub fn load_material_specs() -> Result<&'static LoadedMaterialSpecs> {
    let result = LOADED_MATERIAL_SPECS.get_or_init(load_material_specs_internal);
    match result {
        Ok(specs) => Ok(specs),
        Err(e) => Err(ConfigError::ParseError(e.clone()).into()),
    }
}

/// Application configuration
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Config {
    /// AI backend to use (gemini, claude, codex)
    #[serde(default = "default_backend")]
    pub backend: String,

    /// Model name override (optional)
    #[serde(default)]
    pub model: Option<String>,

    /// Enable caching
    #[serde(default = "default_true")]
    pub cache_enabled: bool,

    /// Cache directory override
    #[serde(default)]
    pub cache_dir: Option<PathBuf>,

    /// Default output format (json, table)
    #[serde(default = "default_output_format")]
    pub output_format: OutputFormat,

    /// Number of ensemble samples for analysis
    #[serde(default = "default_ensemble_count")]
    pub ensemble_count: u32,

    /// Slope factor for effective height reduction
    #[serde(default = "default_slope_factor")]
    pub slope_factor: f64,

    /// Enable local license plate detection/OCR
    #[serde(default = "default_false")]
    pub plate_local_enabled: bool,

    /// Command to run local plate detector (e.g. "python scripts/plate_local.py")
    #[serde(default)]
    pub plate_local_command: Option<String>,

    /// Minimum confidence threshold for local plate detection
    #[serde(default = "default_plate_local_min_conf")]
    pub plate_local_min_conf: f32,

    /// If local detection fails, fall back to API-based stage1
    #[serde(default = "default_true")]
    pub plate_local_fallback_api: bool,
}

fn default_backend() -> String {
    "gemini".to_string()
}

fn default_output_format() -> OutputFormat {
    OutputFormat::Table
}

fn default_ensemble_count() -> u32 {
    1
}

fn default_slope_factor() -> f64 {
    1.0
}

fn default_true() -> bool {
    true
}

fn default_false() -> bool {
    false
}

fn default_plate_local_min_conf() -> f32 {
    0.35
}

impl Default for Config {
    fn default() -> Self {
        Self {
            backend: default_backend(),
            model: None,
            cache_enabled: true,
            cache_dir: None,
            output_format: default_output_format(),
            ensemble_count: default_ensemble_count(),
            slope_factor: default_slope_factor(),
            plate_local_enabled: default_false(),
            plate_local_command: None,
            plate_local_min_conf: default_plate_local_min_conf(),
            plate_local_fallback_api: default_true(),
        }
    }
}

impl Config {
    /// Get the config directory path
    pub fn config_dir() -> Result<PathBuf> {
        let config_dir = dirs::config_dir()
            .ok_or_else(|| ConfigError::NotFound)?
            .join("tonsuu-checker");
        Ok(config_dir)
    }

    /// Get the config file path
    pub fn config_path() -> Result<PathBuf> {
        Ok(Self::config_dir()?.join("config.json"))
    }

    /// Get the cache directory path
    pub fn cache_dir(&self) -> Result<PathBuf> {
        if let Some(ref dir) = self.cache_dir {
            return Ok(dir.clone());
        }

        let cache_dir = dirs::cache_dir()
            .ok_or_else(|| ConfigError::NotFound)?
            .join("tonsuu-checker");
        Ok(cache_dir)
    }

    /// Get the store directory path (for history/feedback data)
    pub fn store_dir(&self) -> Result<PathBuf> {
        let data_dir = dirs::data_dir()
            .ok_or_else(|| ConfigError::NotFound)?
            .join("tonsuu-checker");
        Ok(data_dir)
    }

    /// Load config from file, or create default
    pub fn load() -> Result<Self> {
        let path = Self::config_path()?;

        if path.exists() {
            let content = std::fs::read_to_string(&path)?;
            let config: Config = serde_json::from_str(&content)?;
            Ok(config)
        } else {
            Ok(Config::default())
        }
    }

    /// Save config to file
    pub fn save(&self) -> Result<()> {
        let path = Self::config_path()?;

        // Ensure directory exists
        if let Some(parent) = path.parent() {
            std::fs::create_dir_all(parent)?;
        }

        let content = serde_json::to_string_pretty(self)?;
        std::fs::write(&path, content)?;
        Ok(())
    }
}

impl std::fmt::Display for Config {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        writeln!(f, "Tonsuu Checker Configuration")?;
        writeln!(f, "=============================")?;
        writeln!(f)?;
        writeln!(f, "Backend:        {}", self.backend)?;
        writeln!(
            f,
            "Model:          {}",
            self.model.as_deref().unwrap_or("(default)")
        )?;
        writeln!(f, "Cache enabled:  {}", self.cache_enabled)?;
        writeln!(
            f,
            "Cache dir:      {}",
            self.cache_dir()
                .map(|p| p.display().to_string())
                .unwrap_or_else(|_| "(error)".to_string())
        )?;
        writeln!(f, "Output format:  {}", self.output_format)?;
        writeln!(f, "Ensemble count: {}", self.ensemble_count)?;
        writeln!(f, "Slope factor:   {:.2}", self.slope_factor)?;
        writeln!(
            f,
            "Plate local:    {}",
            if self.plate_local_enabled { "enabled" } else { "disabled" }
        )?;
        writeln!(
            f,
            "Plate command:  {}",
            self.plate_local_command
                .as_deref()
                .unwrap_or("(not set)")
        )?;
        writeln!(f, "Plate min conf: {:.2}", self.plate_local_min_conf)?;
        writeln!(
            f,
            "Plate fallback: {}",
            if self.plate_local_fallback_api { "api" } else { "none" }
        )?;

        if let Ok(path) = Self::config_path() {
            writeln!(f)?;
            writeln!(f, "Config file:    {}", path.display())?;
        }

        Ok(())
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_default_slope_factor() {
        let cfg = Config::default();
        assert_eq!(cfg.slope_factor, 1.0);
    }
}
