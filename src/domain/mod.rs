//! Domain module containing core business types and services

pub mod model;
pub mod repository;
pub mod service;

pub use model::*;
pub use repository::{
    AnalysisHistoryRepository, VehicleRepository,
};
