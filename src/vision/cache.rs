//! Cache for analysis results

use crate::error::Result;
use crate::types::EstimationResult;
use sha2::{Digest, Sha256};
use std::fs::{self, File};
use std::io::{self, BufReader};
use std::path::{Path, PathBuf};

/// Cache manager for analysis results
pub struct Cache {
    cache_dir: PathBuf,
}

impl Cache {
    /// Create a new cache manager
    pub fn new(cache_dir: PathBuf) -> Result<Self> {
        fs::create_dir_all(&cache_dir)?;
        Ok(Self { cache_dir })
    }

    /// Get cache key for an image file (streaming hash for memory efficiency)
    fn cache_key(image_path: &Path) -> Result<String> {
        let file = File::open(image_path)?;
        let mut reader = BufReader::new(file);
        let mut hasher = Sha256::new();
        io::copy(&mut reader, &mut hasher)?;
        let hash = hasher.finalize();
        Ok(format!("{:x}", hash))
    }

    /// Get cached result for an image
    pub fn get(&self, image_path: &Path) -> Result<Option<EstimationResult>> {
        let key = Self::cache_key(image_path)?;
        let cache_path = self.cache_dir.join(format!("{}.json", key));

        if !cache_path.exists() {
            return Ok(None);
        }

        let content = fs::read_to_string(&cache_path)?;
        let result: EstimationResult = serde_json::from_str(&content)?;
        Ok(Some(result))
    }

    /// Store result in cache
    pub fn set(&self, image_path: &Path, result: &EstimationResult) -> Result<()> {
        let key = Self::cache_key(image_path)?;
        let cache_path = self.cache_dir.join(format!("{}.json", key));

        let content = serde_json::to_string_pretty(result)?;
        fs::write(&cache_path, content)?;
        Ok(())
    }

    /// Clear all cached results
    pub fn clear(&self) -> Result<usize> {
        let mut count = 0;

        for entry in fs::read_dir(&self.cache_dir)? {
            let entry = entry?;
            let path = entry.path();
            if path.extension().map_or(false, |e| e == "json") {
                fs::remove_file(&path)?;
                count += 1;
            }
        }

        Ok(count)
    }

    /// Get cache statistics
    pub fn stats(&self) -> Result<CacheStats> {
        let mut count = 0;
        let mut total_size = 0u64;

        for entry in fs::read_dir(&self.cache_dir)? {
            let entry = entry?;
            let path = entry.path();
            if path.extension().map_or(false, |e| e == "json") {
                count += 1;
                if let Ok(metadata) = fs::metadata(&path) {
                    total_size += metadata.len();
                }
            }
        }

        Ok(CacheStats {
            entry_count: count,
            total_size_bytes: total_size,
            cache_dir: self.cache_dir.clone(),
        })
    }
}

/// Cache statistics
#[derive(Debug)]
pub struct CacheStats {
    pub entry_count: usize,
    pub total_size_bytes: u64,
    pub cache_dir: PathBuf,
}

impl CacheStats {
    pub fn display(&self) -> String {
        let size_kb = self.total_size_bytes as f64 / 1024.0;
        format!(
            "Cache Statistics\n\
             ================\n\
             Entries:    {}\n\
             Total size: {:.2} KB\n\
             Location:   {}",
            self.entry_count,
            size_kb,
            self.cache_dir.display()
        )
    }
}
