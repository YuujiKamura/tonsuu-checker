#![allow(unused_imports)]
//! Constants for tonnage estimation

pub mod materials;
pub mod truck_specs;
pub mod weight_calculator;

/// Re-export prompts from vision module for backwards compatibility
/// This module is deprecated. Please use `crate::vision::ai::prompts` instead.
pub mod prompts {
    pub use crate::vision::ai::prompts::*;
}

pub use materials::get_material_spec;
pub use truck_specs::get_truck_spec;
pub use weight_calculator::{calculate_weight, calculate_weight_explicit};
