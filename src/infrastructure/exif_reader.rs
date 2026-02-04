//! EXIF metadata reader for photos
//!
//! Extracts capture datetime and other metadata from image files.

#![allow(dead_code)]

use std::fs::File;
use std::io::BufReader;
use std::path::Path;

use chrono::{DateTime, NaiveDateTime, Utc};
use exif::{In, Reader, Tag};

/// EXIF metadata extracted from an image
#[derive(Debug, Clone)]
pub struct PhotoMetadata {
    /// Original capture datetime (from camera)
    pub captured_at: Option<DateTime<Utc>>,
    /// Camera make (e.g., "Apple", "Canon")
    pub camera_make: Option<String>,
    /// Camera model (e.g., "iPhone 14 Pro")
    pub camera_model: Option<String>,
    /// GPS latitude
    pub latitude: Option<f64>,
    /// GPS longitude
    pub longitude: Option<f64>,
}

impl PhotoMetadata {
    /// Read EXIF metadata from an image file
    pub fn from_file(path: &Path) -> Option<Self> {
        let file = File::open(path).ok()?;
        let mut bufreader = BufReader::new(file);
        let exif = Reader::new().read_from_container(&mut bufreader).ok()?;

        let captured_at = exif
            .get_field(Tag::DateTimeOriginal, In::PRIMARY)
            .or_else(|| exif.get_field(Tag::DateTime, In::PRIMARY))
            .and_then(|f| parse_exif_datetime(&f.display_value().to_string()));

        let camera_make = exif
            .get_field(Tag::Make, In::PRIMARY)
            .map(|f| f.display_value().to_string().trim().to_string());

        let camera_model = exif
            .get_field(Tag::Model, In::PRIMARY)
            .map(|f| f.display_value().to_string().trim().to_string());

        let latitude = extract_gps_coord(&exif, Tag::GPSLatitude, Tag::GPSLatitudeRef);
        let longitude = extract_gps_coord(&exif, Tag::GPSLongitude, Tag::GPSLongitudeRef);

        Some(Self {
            captured_at,
            camera_make,
            camera_model,
            latitude,
            longitude,
        })
    }

    /// Get capture datetime, falling back to file modification time
    pub fn captured_at_or_file_time(path: &Path) -> Option<DateTime<Utc>> {
        // Try EXIF first
        if let Some(meta) = Self::from_file(path) {
            if let Some(dt) = meta.captured_at {
                return Some(dt);
            }
        }

        // Fallback to file modification time
        std::fs::metadata(path)
            .ok()
            .and_then(|m| m.modified().ok())
            .map(|t| DateTime::<Utc>::from(t))
    }
}

/// Parse EXIF datetime string (format: "2024:01:15 10:30:45")
fn parse_exif_datetime(s: &str) -> Option<DateTime<Utc>> {
    // Remove quotes if present
    let s = s.trim().trim_matches('"');

    // Try common EXIF datetime format
    NaiveDateTime::parse_from_str(s, "%Y:%m:%d %H:%M:%S")
        .ok()
        .map(|dt| dt.and_utc())
}

/// Extract GPS coordinate from EXIF
fn extract_gps_coord(exif: &exif::Exif, coord_tag: Tag, ref_tag: Tag) -> Option<f64> {
    let coord_field = exif.get_field(coord_tag, In::PRIMARY)?;
    let ref_field = exif.get_field(ref_tag, In::PRIMARY)?;

    // Parse degrees, minutes, seconds
    let value = coord_field.display_value().to_string();
    let parts: Vec<f64> = value
        .split(|c: char| !c.is_numeric() && c != '.')
        .filter(|s| !s.is_empty())
        .filter_map(|s| s.parse().ok())
        .collect();

    if parts.len() >= 3 {
        let degrees = parts[0] + parts[1] / 60.0 + parts[2] / 3600.0;

        // Apply sign based on reference (N/S, E/W)
        let ref_str = ref_field.display_value().to_string();
        let sign = if ref_str.contains('S') || ref_str.contains('W') {
            -1.0
        } else {
            1.0
        };

        Some(degrees * sign)
    } else {
        None
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use chrono::Datelike;

    #[test]
    fn test_parse_exif_datetime() {
        let dt = parse_exif_datetime("2024:01:15 10:30:45").unwrap();
        assert_eq!(dt.year(), 2024);
        assert_eq!(dt.month(), 1);
        assert_eq!(dt.day(), 15);
    }

    #[test]
    fn test_parse_exif_datetime_with_quotes() {
        let dt = parse_exif_datetime("\"2024:01:15 10:30:45\"").unwrap();
        assert_eq!(dt.year(), 2024);
    }
}
