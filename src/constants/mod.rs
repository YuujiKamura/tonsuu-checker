#![allow(unused_imports)]
//! Constants for tonnage estimation

pub mod materials;
pub mod truck_specs;
pub mod weight_calculator;


pub use materials::get_material_spec;
pub use truck_specs::get_truck_spec;
pub use weight_calculator::{calculate_weight, calculate_weight_explicit};
