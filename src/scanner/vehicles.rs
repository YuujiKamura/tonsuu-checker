//! Vehicle folder scanning for auto-collecting vehicle images
//!
//! This module provides functionality to scan a folder structure where each
//! subfolder represents a vehicle, and automatically identifies potential
//! 車検証 (vehicle inspection certificate) images vs regular vehicle photos.

use crate::error::{Error, Result};
use std::collections::HashMap;
use std::path::{Path, PathBuf};
use walkdir::WalkDir;

/// Supported image extensions for scanning (limited to common formats)
const SCAN_IMAGE_EXTENSIONS: &[&str] = &["jpg", "jpeg", "png"];

/// Keywords that indicate a 車検証 (vehicle inspection certificate) image
const SHAKEN_KEYWORDS: &[&str] = &[
    "車検",   // shaken (vehicle inspection)
    "shaken", // romanized
    "検査",   // inspection
    "証",     // certificate
];

/// Keywords that indicate a vehicle photo
const PHOTO_KEYWORDS: &[&str] = &[
    "車両",   // vehicle
    "truck",  // truck
    "photo",  // photo
    "写真",   // photo (Japanese)
    "外観",   // exterior
    "荷台",   // cargo bed
];

/// Result of scanning a single vehicle folder
#[derive(Debug, Clone)]
pub struct VehicleFolderScan {
    /// Name of the folder (typically vehicle identifier)
    pub folder_name: String,
    /// Full path to the folder
    pub folder_path: PathBuf,
    /// Potential 車検証 (vehicle inspection certificate) images
    pub shaken_candidates: Vec<PathBuf>,
    /// Vehicle photos (non-車検証 images)
    pub photo_candidates: Vec<PathBuf>,
}

impl VehicleFolderScan {
    /// Create a new VehicleFolderScan
    fn new(folder_name: String, folder_path: PathBuf) -> Self {
        Self {
            folder_name,
            folder_path,
            shaken_candidates: Vec::new(),
            photo_candidates: Vec::new(),
        }
    }

    /// Total number of images in this folder
    pub fn total_images(&self) -> usize {
        self.shaken_candidates.len() + self.photo_candidates.len()
    }

    /// Check if this folder has any images
    pub fn has_images(&self) -> bool {
        self.total_images() > 0
    }

    /// Get the primary 車検証 candidate (first one if available)
    pub fn primary_shaken(&self) -> Option<&PathBuf> {
        self.shaken_candidates.first()
    }
}

/// Result of scanning multiple vehicle folders
#[derive(Debug, Clone)]
pub struct FolderScanResult {
    /// List of vehicle folder scans
    pub vehicles: Vec<VehicleFolderScan>,
    /// Total number of images found across all folders
    pub total_images: usize,
}

impl FolderScanResult {
    /// Create a new empty FolderScanResult
    fn new() -> Self {
        Self {
            vehicles: Vec::new(),
            total_images: 0,
        }
    }

    /// Number of vehicles found
    pub fn vehicle_count(&self) -> usize {
        self.vehicles.len()
    }

    /// Check if any vehicles were found
    pub fn is_empty(&self) -> bool {
        self.vehicles.is_empty()
    }

    /// Get vehicles that have 車検証 candidates
    pub fn vehicles_with_shaken(&self) -> Vec<&VehicleFolderScan> {
        self.vehicles
            .iter()
            .filter(|v| !v.shaken_candidates.is_empty())
            .collect()
    }
}

/// Classification result for an image file
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum ImageClassification {
    /// Likely a 車検証 (vehicle inspection certificate)
    Shaken,
    /// Likely a vehicle photo
    Photo,
    /// Unknown classification
    Unknown,
}

/// Check if a file extension is a supported scan image
fn is_scan_image(path: &Path) -> bool {
    path.extension()
        .and_then(|ext| ext.to_str())
        .map(|ext| SCAN_IMAGE_EXTENSIONS.contains(&ext.to_lowercase().as_str()))
        .unwrap_or(false)
}

/// Classify an image based on its filename
fn classify_image_by_name(path: &Path) -> ImageClassification {
    let filename = path
        .file_name()
        .and_then(|n| n.to_str())
        .map(|s| s.to_lowercase())
        .unwrap_or_default();

    // Check for 車検証 keywords first (higher priority)
    for keyword in SHAKEN_KEYWORDS {
        if filename.contains(keyword) {
            return ImageClassification::Shaken;
        }
    }

    // Check for photo keywords
    for keyword in PHOTO_KEYWORDS {
        if filename.contains(keyword) {
            return ImageClassification::Photo;
        }
    }

    ImageClassification::Unknown
}

/// Scan a vehicle folder for images and classify them
///
/// # Arguments
///
/// * `root_path` - The root folder to scan. Each immediate subfolder is treated
///                 as a vehicle folder.
///
/// # Returns
///
/// A `FolderScanResult` containing all found vehicles and their images.
///
/// # Errors
///
/// Returns an error if:
/// - The root path does not exist
/// - The root path is not a directory
/// - There are permission issues reading the directory
///
/// # Example
///
/// ```ignore
/// use std::path::Path;
/// use tonsuu_checker::scanner::vehicles::scan_vehicle_folder;
///
/// let result = scan_vehicle_folder(Path::new("./vehicles"))?;
/// println!("Found {} vehicles with {} total images",
///          result.vehicle_count(), result.total_images);
/// ```
pub fn scan_vehicle_folder(root_path: &Path) -> Result<FolderScanResult> {
    // Validate root path
    if !root_path.exists() {
        return Err(Error::FileNotFound(root_path.display().to_string()));
    }

    if !root_path.is_dir() {
        return Err(Error::InvalidImageFormat(format!(
            "{} is not a directory",
            root_path.display()
        )));
    }

    let mut result = FolderScanResult::new();

    // Group images by their parent folder
    let mut folder_images: HashMap<PathBuf, Vec<PathBuf>> = HashMap::new();

    // Walk the directory tree
    for entry in WalkDir::new(root_path)
        .follow_links(true)
        .min_depth(1) // Skip root folder itself
        .into_iter()
        .filter_map(|e| e.ok())
    {
        let path = entry.path();

        // Only process image files
        if !path.is_file() || !is_scan_image(path) {
            continue;
        }

        // Get the immediate subfolder of root (the vehicle folder)
        let vehicle_folder = get_vehicle_folder(root_path, path);

        if let Some(folder) = vehicle_folder {
            folder_images
                .entry(folder)
                .or_default()
                .push(path.to_path_buf());
        }
    }

    // Process each vehicle folder
    for (folder_path, mut images) in folder_images {
        let folder_name = folder_path
            .file_name()
            .and_then(|n| n.to_str())
            .unwrap_or("unknown")
            .to_string();

        let mut vehicle_scan = VehicleFolderScan::new(folder_name, folder_path);

        // Sort images by filename for consistent ordering
        images.sort_by(|a, b| {
            a.file_name()
                .and_then(|n| n.to_str())
                .unwrap_or("")
                .cmp(b.file_name().and_then(|n| n.to_str()).unwrap_or(""))
        });

        // Track if we've found any explicit shaken images
        let mut has_explicit_shaken = false;

        // First pass: classify images by name
        for image in &images {
            match classify_image_by_name(image) {
                ImageClassification::Shaken => {
                    vehicle_scan.shaken_candidates.push(image.clone());
                    has_explicit_shaken = true;
                }
                ImageClassification::Photo => {
                    vehicle_scan.photo_candidates.push(image.clone());
                }
                ImageClassification::Unknown => {
                    // Will be processed in second pass
                }
            }
        }

        // Second pass: handle unknown images
        for image in &images {
            if classify_image_by_name(image) == ImageClassification::Unknown {
                // If no explicit shaken found and this is the first image,
                // treat it as a potential shaken candidate
                if !has_explicit_shaken && vehicle_scan.shaken_candidates.is_empty() {
                    vehicle_scan.shaken_candidates.push(image.clone());
                    has_explicit_shaken = true;
                } else {
                    // Otherwise, treat as a photo
                    vehicle_scan.photo_candidates.push(image.clone());
                }
            }
        }

        result.total_images += vehicle_scan.total_images();

        // Only add folders that have images
        if vehicle_scan.has_images() {
            result.vehicles.push(vehicle_scan);
        }
    }

    // Sort vehicles by folder name for consistent ordering
    result
        .vehicles
        .sort_by(|a, b| a.folder_name.cmp(&b.folder_name));

    Ok(result)
}

/// Get the vehicle folder path for a given image path
///
/// The vehicle folder is the immediate subfolder of the root path.
/// For example, if root is "/vehicles" and image is "/vehicles/truck1/photos/img.jpg",
/// the vehicle folder is "/vehicles/truck1".
fn get_vehicle_folder(root: &Path, image_path: &Path) -> Option<PathBuf> {
    // Get the relative path from root
    let relative = image_path.strip_prefix(root).ok()?;

    // Get the first component (the vehicle folder name)
    let first_component = relative.components().next()?;

    // Return the full path to the vehicle folder
    Some(root.join(first_component))
}

/// Scan a single folder (non-recursive) for vehicle images
///
/// This is useful when you want to scan a single vehicle folder directly
/// without treating subfolders as separate vehicles.
///
/// # Arguments
///
/// * `folder_path` - The folder to scan for images
///
/// # Returns
///
/// A `VehicleFolderScan` for the specified folder.
pub fn scan_single_folder(folder_path: &Path) -> Result<VehicleFolderScan> {
    if !folder_path.exists() {
        return Err(Error::FileNotFound(folder_path.display().to_string()));
    }

    if !folder_path.is_dir() {
        return Err(Error::InvalidImageFormat(format!(
            "{} is not a directory",
            folder_path.display()
        )));
    }

    let folder_name = folder_path
        .file_name()
        .and_then(|n| n.to_str())
        .unwrap_or("unknown")
        .to_string();

    let mut vehicle_scan = VehicleFolderScan::new(folder_name, folder_path.to_path_buf());

    // Collect all images in the folder (non-recursive)
    let mut images: Vec<PathBuf> = std::fs::read_dir(folder_path)?
        .filter_map(|e| e.ok())
        .map(|e| e.path())
        .filter(|p| p.is_file() && is_scan_image(p))
        .collect();

    // Sort by filename
    images.sort_by(|a, b| {
        a.file_name()
            .and_then(|n| n.to_str())
            .unwrap_or("")
            .cmp(b.file_name().and_then(|n| n.to_str()).unwrap_or(""))
    });

    // Classify images
    let mut has_explicit_shaken = false;

    for image in &images {
        match classify_image_by_name(image) {
            ImageClassification::Shaken => {
                vehicle_scan.shaken_candidates.push(image.clone());
                has_explicit_shaken = true;
            }
            ImageClassification::Photo => {
                vehicle_scan.photo_candidates.push(image.clone());
            }
            ImageClassification::Unknown => {}
        }
    }

    // Handle unknown images
    for image in &images {
        if classify_image_by_name(image) == ImageClassification::Unknown {
            if !has_explicit_shaken && vehicle_scan.shaken_candidates.is_empty() {
                vehicle_scan.shaken_candidates.push(image.clone());
                has_explicit_shaken = true;
            } else {
                vehicle_scan.photo_candidates.push(image.clone());
            }
        }
    }

    Ok(vehicle_scan)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_classify_shaken_keywords() {
        assert_eq!(
            classify_image_by_name(Path::new("車検証.jpg")),
            ImageClassification::Shaken
        );
        assert_eq!(
            classify_image_by_name(Path::new("shaken_001.png")),
            ImageClassification::Shaken
        );
        assert_eq!(
            classify_image_by_name(Path::new("検査票.jpg")),
            ImageClassification::Shaken
        );
        assert_eq!(
            classify_image_by_name(Path::new("登録証.jpg")),
            ImageClassification::Shaken
        );
    }

    #[test]
    fn test_classify_photo_keywords() {
        assert_eq!(
            classify_image_by_name(Path::new("車両_front.jpg")),
            ImageClassification::Photo
        );
        assert_eq!(
            classify_image_by_name(Path::new("truck_001.png")),
            ImageClassification::Photo
        );
        assert_eq!(
            classify_image_by_name(Path::new("photo_side.jpg")),
            ImageClassification::Photo
        );
        assert_eq!(
            classify_image_by_name(Path::new("写真.jpg")),
            ImageClassification::Photo
        );
    }

    #[test]
    fn test_classify_unknown() {
        assert_eq!(
            classify_image_by_name(Path::new("IMG_001.jpg")),
            ImageClassification::Unknown
        );
        assert_eq!(
            classify_image_by_name(Path::new("DSC_1234.png")),
            ImageClassification::Unknown
        );
    }

    #[test]
    fn test_is_scan_image() {
        assert!(is_scan_image(Path::new("test.jpg")));
        assert!(is_scan_image(Path::new("test.JPEG")));
        assert!(is_scan_image(Path::new("test.png")));
        assert!(!is_scan_image(Path::new("test.gif")));
        assert!(!is_scan_image(Path::new("test.txt")));
    }

    #[test]
    fn test_vehicle_folder_scan_helpers() {
        let mut scan = VehicleFolderScan::new("test".to_string(), PathBuf::from("/test"));
        assert!(!scan.has_images());
        assert_eq!(scan.total_images(), 0);
        assert!(scan.primary_shaken().is_none());

        scan.shaken_candidates.push(PathBuf::from("/test/shaken.jpg"));
        scan.photo_candidates.push(PathBuf::from("/test/photo.jpg"));

        assert!(scan.has_images());
        assert_eq!(scan.total_images(), 2);
        assert!(scan.primary_shaken().is_some());
    }

    #[test]
    fn test_folder_scan_result_helpers() {
        let mut result = FolderScanResult::new();
        assert!(result.is_empty());
        assert_eq!(result.vehicle_count(), 0);

        let mut scan = VehicleFolderScan::new("test".to_string(), PathBuf::from("/test"));
        scan.shaken_candidates.push(PathBuf::from("/test/shaken.jpg"));
        result.vehicles.push(scan);
        result.total_images = 1;

        assert!(!result.is_empty());
        assert_eq!(result.vehicle_count(), 1);
        assert_eq!(result.vehicles_with_shaken().len(), 1);
    }

    #[test]
    fn test_get_vehicle_folder() {
        let root = Path::new("/vehicles");

        // Direct subfolder
        let path = Path::new("/vehicles/truck1/image.jpg");
        assert_eq!(
            get_vehicle_folder(root, path),
            Some(PathBuf::from("/vehicles/truck1"))
        );

        // Nested subfolder
        let path = Path::new("/vehicles/truck1/photos/image.jpg");
        assert_eq!(
            get_vehicle_folder(root, path),
            Some(PathBuf::from("/vehicles/truck1"))
        );

        // Image directly in root (should return None for first component)
        let root = Path::new("/vehicles");
        let path = Path::new("/other/image.jpg");
        assert_eq!(get_vehicle_folder(root, path), None);
    }
}
