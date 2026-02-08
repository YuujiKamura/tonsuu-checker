//! Domain services

pub mod overload_checker;
pub mod weight_calculator;

pub use overload_checker::{
    check_overloads, generate_overload_report, OverloadCheckResult,
};
