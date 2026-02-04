//! Domain services
//!
//! This module contains business logic services for the domain layer.

pub mod overload_checker;
pub mod weight_calculator;

pub use overload_checker::{
    check_overloads, generate_overload_report, load_slips_from_csv, load_vehicles_from_csv,
};
