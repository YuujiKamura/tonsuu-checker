//! Image scanning and validation

#![allow(dead_code)]

pub mod vehicles;

use crate::error::{Error, Result};
use std::path::{Path, PathBuf};
use walkdir::WalkDir;


/// Supported image extensions
const IMAGE_EXTENSIONS: &[&str] = &["jpg", "jpeg", "png", "webp", "gif", "bmp"];

/// Check if a path is a supported image file
pub fn is_supported_image(path: &Path) -> bool {
    path.extension()
        .and_then(|ext| ext.to_str())
        .map(|ext| IMAGE_EXTENSIONS.contains(&ext.to_lowercase().as_str()))
        .unwrap_or(false)
}

/// Validate an image file exists and is readable
pub fn validate_image(path: &Path) -> Result<()> {
    if !path.exists() {
        return Err(Error::FileNotFound(path.display().to_string()));
    }

    if !path.is_file() {
        return Err(Error::InvalidImageFormat(format!(
            "{} is not a file",
            path.display()
        )));
    }

    if !is_supported_image(path) {
        return Err(Error::InvalidImageFormat(format!(
            "Unsupported image format: {}",
            path.display()
        )));
    }

    // Try to open the image to validate it
    image::open(path)?;

    Ok(())
}

/// Scan a directory for image files
pub fn scan_directory(dir: &Path) -> Result<Vec<PathBuf>> {
    if !dir.exists() {
        return Err(Error::FileNotFound(dir.display().to_string()));
    }

    if !dir.is_dir() {
        return Err(Error::InvalidImageFormat(format!(
            "{} is not a directory",
            dir.display()
        )));
    }

    let mut images = Vec::new();

    for entry in WalkDir::new(dir)
        .follow_links(true)
        .into_iter()
        .filter_map(|e| e.ok())
    {
        let path = entry.path();
        if path.is_file() && is_supported_image(path) {
            images.push(path.to_path_buf());
        }
    }

    // Sort by filename for consistent ordering
    images.sort_by(|a, b| {
        a.file_name()
            .and_then(|n| n.to_str())
            .unwrap_or("")
            .cmp(b.file_name().and_then(|n| n.to_str()).unwrap_or(""))
    });

    Ok(images)
}

/// Get image dimensions
pub fn get_image_dimensions(path: &Path) -> Result<(u32, u32)> {
    let img = image::open(path)?;
    Ok((img.width(), img.height()))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_is_supported_image() {
        assert!(is_supported_image(Path::new("test.jpg")));
        assert!(is_supported_image(Path::new("test.JPEG")));
        assert!(is_supported_image(Path::new("test.png")));
        assert!(!is_supported_image(Path::new("test.txt")));
        assert!(!is_supported_image(Path::new("test")));
    }
}
