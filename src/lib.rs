//! Tonsuu Checker Library
//!
//! Dump truck cargo weight estimation using AI image analysis.

pub mod app;
pub mod cli;
pub mod commands;
pub mod config;
pub mod constants;
pub mod domain;
pub mod error;
pub mod export;
pub mod infrastructure;
pub mod output;
pub mod scanner;
pub mod store;
pub mod types;
pub mod vision;

/// Backwards-compat shim for legacy imports (tests, older callers)
pub mod analyzer {
    pub use crate::vision::*;
}

/// Re-export plate_local for backwards compatibility
/// This module is deprecated. Please use `crate::vision::plate_recognizer` instead.
pub mod plate_local {
    pub use crate::vision::plate_recognizer::*;
}
