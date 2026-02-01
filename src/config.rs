//! Configuration management for tonsuu-checker
//!
//! Config stored at: ~/.config/tonsuu-checker/config.json

use crate::cli::OutputFormat;
use crate::error::{Error, Result};
use serde::{Deserialize, Serialize};
use std::path::PathBuf;

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

fn default_true() -> bool {
    true
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
        }
    }
}

impl Config {
    /// Get the config directory path
    pub fn config_dir() -> Result<PathBuf> {
        let config_dir = dirs::config_dir()
            .ok_or_else(|| Error::Config("Could not determine config directory".to_string()))?
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
            .ok_or_else(|| Error::Config("Could not determine cache directory".to_string()))?
            .join("tonsuu-checker");
        Ok(cache_dir)
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

    /// Display config as formatted string
    pub fn display(&self) -> String {
        let mut output = String::new();
        output.push_str("Tonsuu Checker Configuration\n");
        output.push_str("============================\n\n");
        output.push_str(&format!("Backend:        {}\n", self.backend));
        output.push_str(&format!(
            "Model:          {}\n",
            self.model.as_deref().unwrap_or("(default)")
        ));
        output.push_str(&format!("Cache enabled:  {}\n", self.cache_enabled));
        output.push_str(&format!(
            "Cache dir:      {}\n",
            self.cache_dir()
                .map(|p| p.display().to_string())
                .unwrap_or_else(|_| "(error)".to_string())
        ));
        output.push_str(&format!("Output format:  {}\n", self.output_format));
        output.push_str(&format!("Ensemble count: {}\n", self.ensemble_count));

        if let Ok(path) = Self::config_path() {
            output.push_str(&format!("\nConfig file:    {}\n", path.display()));
        }

        output
    }
}
