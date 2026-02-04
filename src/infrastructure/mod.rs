//! Infrastructure layer
//!
//! This module contains concrete implementations of domain interfaces,
//! including persistence mechanisms, external service integrations, etc.

pub mod csv_loader;
pub mod exif_reader;
pub mod legacy_importer;
pub mod persistence;
pub mod vehicle_master_loader;

