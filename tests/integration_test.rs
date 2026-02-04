//! Integration tests for tonsuu-checker analysis

use std::path::PathBuf;
use tempfile::tempdir;
use tonsuu_checker::analyzer::{analyze_image, analyze_image_staged, AnalyzerConfig, StagedAnalysisOptions};
use tonsuu_checker::constants::get_truck_spec;
use tonsuu_checker::store::Store;
use tonsuu_checker::types::TruckClass;

fn test_image_path() -> PathBuf {
    PathBuf::from(env!("CARGO_MANIFEST_DIR"))
        .join("tests")
        .join("fixtures")
        .join("test_truck.jpg")
}

/// Test that analysis completes without error
#[test]
#[ignore] // Run with: cargo test --release -- --ignored
fn test_analysis_completes() {
    let image_path = test_image_path();
    assert!(image_path.exists(), "Test image not found: {:?}", image_path);

    let config = AnalyzerConfig::default();

    let result = analyze_image(&image_path, &config);

    assert!(result.is_ok(), "Analysis failed: {:?}", result.err());

    let estimation = result.unwrap();
    println!("=== Analysis Result ===");
    println!("Target detected: {}", estimation.is_target_detected);
    println!("Truck type: {}", estimation.truck_type);
    println!("Material: {}", estimation.material_type);
    println!("Volume: {:.2} m³", estimation.estimated_volume_m3);
    println!("Tonnage: {:.2} t", estimation.estimated_tonnage);
    println!("Confidence: {:.1}%", estimation.confidence_score * 100.0);
    println!("Reasoning: {}", estimation.reasoning);
}

/// Test with Claude backend
#[test]
#[ignore]
fn test_analysis_claude() {
    let image_path = test_image_path();
    assert!(image_path.exists(), "Test image not found: {:?}", image_path);

    let config = AnalyzerConfig::default().with_backend("claude");

    let result = analyze_image(&image_path, &config);

    assert!(result.is_ok(), "Claude analysis failed: {:?}", result.err());

    let estimation = result.unwrap();
    assert!(estimation.is_target_detected, "Target should be detected");
    println!("Claude result: {:.2} t ({})", estimation.estimated_tonnage, estimation.truck_type);
}

/// Test with Gemini backend (default model)
#[test]
#[ignore]
fn test_analysis_gemini() {
    let image_path = test_image_path();
    assert!(image_path.exists(), "Test image not found: {:?}", image_path);

    // Use default model (no specific model set)
    let config = AnalyzerConfig::default().with_backend("gemini");

    let result = analyze_image(&image_path, &config);

    assert!(result.is_ok(), "Gemini analysis failed: {:?}", result.err());

    let estimation = result.unwrap();
    println!("=== Gemini Analysis Result ===");
    println!("Target detected: {}", estimation.is_target_detected);
    println!("Truck type: {}", estimation.truck_type);
    println!("Material: {}", estimation.material_type);
    println!("Volume: {:.2} m³", estimation.estimated_volume_m3);
    println!("Tonnage: {:.2} t", estimation.estimated_tonnage);
    println!("Confidence: {:.1}%", estimation.confidence_score * 100.0);
}

/// Test result has valid values
#[test]
#[ignore]
fn test_result_validity() {
    let image_path = test_image_path();
    let config = AnalyzerConfig::default().with_backend("claude");

    let result = analyze_image(&image_path, &config).expect("Analysis should succeed");

    // Check valid ranges
    assert!(result.estimated_tonnage >= 0.0, "Tonnage should be non-negative");
    assert!(result.estimated_tonnage <= 20.0, "Tonnage should be reasonable (<20t)");
    assert!(result.estimated_volume_m3 >= 0.0, "Volume should be non-negative");
    assert!(result.confidence_score >= 0.0 && result.confidence_score <= 1.0,
            "Confidence should be 0-1");

    // Check required fields are populated
    assert!(!result.truck_type.is_empty(), "Truck type should be set");
    assert!(!result.material_type.is_empty(), "Material type should be set");
}

/// Test staged analysis with max capacity
#[test]
#[ignore]
fn test_staged_analysis_with_capacity() {
    let image_path = test_image_path();
    assert!(image_path.exists(), "Test image not found: {:?}", image_path);

    // Create temporary store
    let temp_dir = tempdir().expect("Failed to create temp dir");
    let store = Store::open(temp_dir.path().to_path_buf()).expect("Failed to open store");

    let config = AnalyzerConfig::default().with_backend("gemini");
    let options = StagedAnalysisOptions::default()
        .with_truck_class(TruckClass::TenTon)  // 10t truck
        .with_ensemble_count(1);

    let result = analyze_image_staged(&image_path, &config, &options, &store, None);

    assert!(result.is_ok(), "Staged analysis failed: {:?}", result.err());

    let estimation = result.unwrap();
    println!("=== Staged Analysis Result (10t class) ===");
    println!("Target detected: {}", estimation.is_target_detected);
    println!("Truck type: {}", estimation.truck_type);
    println!("Material: {}", estimation.material_type);
    println!("Volume: {:.2} m³", estimation.estimated_volume_m3);
    println!("Tonnage: {:.2} t", estimation.estimated_tonnage);
    println!("Confidence: {:.1}%", estimation.confidence_score * 100.0);
    println!("Max capacity (from spec): {:?}", get_truck_spec(&estimation.truck_type).map(|s| s.max_capacity));
}

/// Test staged analysis without max capacity (auto-detect)
#[test]
#[ignore]
fn test_staged_analysis_auto_detect() {
    let image_path = test_image_path();
    assert!(image_path.exists(), "Test image not found: {:?}", image_path);

    // Create temporary store
    let temp_dir = tempdir().expect("Failed to create temp dir");
    let store = Store::open(temp_dir.path().to_path_buf()).expect("Failed to open store");

    let config = AnalyzerConfig::default().with_backend("gemini");
    let options = StagedAnalysisOptions::default()
        .with_ensemble_count(2);  // 2 iterations

    let result = analyze_image_staged(&image_path, &config, &options, &store, None);

    assert!(result.is_ok(), "Staged analysis failed: {:?}", result.err());

    let estimation = result.unwrap();
    println!("=== Staged Analysis Result (auto-detect) ===");
    println!("Target detected: {}", estimation.is_target_detected);
    println!("Truck type: {}", estimation.truck_type);

    // Check that truck class was detected from truck type
    if let Some(spec) = get_truck_spec(&estimation.truck_type) {
        println!("Max capacity (from spec): {:?}", spec.max_capacity);
        let detected_class = TruckClass::from_capacity(spec.max_capacity);
        println!("Detected truck class: {}", detected_class.label());
        assert_ne!(detected_class, TruckClass::Unknown, "Should detect truck class");
    }

    println!("Tonnage: {:.2} t", estimation.estimated_tonnage);
    println!("Ensemble count: {:?}", estimation.ensemble_count);
}

/// Test truck class detection
#[test]
fn test_truck_class_from_capacity() {
    assert_eq!(TruckClass::from_capacity(2.0), TruckClass::TwoTon);
    assert_eq!(TruckClass::from_capacity(4.0), TruckClass::FourTon);
    assert_eq!(TruckClass::from_capacity(6.5), TruckClass::IncreasedTon);
    assert_eq!(TruckClass::from_capacity(10.0), TruckClass::TenTon);
    assert_eq!(TruckClass::from_capacity(0.5), TruckClass::Unknown);
    assert_eq!(TruckClass::from_capacity(15.0), TruckClass::Unknown);
}

/// Test vehicle store CRUD operations
#[test]
fn test_vehicle_store() {
    use tonsuu_checker::store::VehicleStore;
    use tonsuu_checker::types::RegisteredVehicle;

    let temp_dir = tempdir().expect("Failed to create temp dir");
    let mut store = VehicleStore::open(temp_dir.path().to_path_buf())
        .expect("Failed to open vehicle store");

    // Initially empty
    assert_eq!(store.count(), 0);
    assert!(store.all_vehicles().is_empty());

    // Add a vehicle
    let vehicle = RegisteredVehicle::new("日野 プロフィア".to_string(), 10.0)
        .with_license_plate("品川 100 あ 1234".to_string());

    let id = store.add_vehicle(vehicle.clone()).expect("Failed to add vehicle");
    assert_eq!(store.count(), 1);

    // Get by ID
    let retrieved = store.get_vehicle(&id).expect("Vehicle not found");
    assert_eq!(retrieved.name, "日野 プロフィア");
    assert_eq!(retrieved.max_capacity, 10.0);
    assert_eq!(retrieved.truck_class(), TruckClass::TenTon);

    // Get by license plate
    let by_plate = store.get_by_license_plate("品川 100 あ 1234")
        .expect("Vehicle not found by plate");
    assert_eq!(by_plate.id, id);

    // Add another vehicle
    let vehicle2 = RegisteredVehicle::new("いすゞ エルフ".to_string(), 2.0);
    store.add_vehicle(vehicle2).expect("Failed to add second vehicle");
    assert_eq!(store.count(), 2);

    // Filter by class
    let ten_ton_vehicles = store.vehicles_by_class(TruckClass::TenTon);
    assert_eq!(ten_ton_vehicles.len(), 1);
    assert_eq!(ten_ton_vehicles[0].name, "日野 プロフィア");

    let two_ton_vehicles = store.vehicles_by_class(TruckClass::TwoTon);
    assert_eq!(two_ton_vehicles.len(), 1);
    assert_eq!(two_ton_vehicles[0].name, "いすゞ エルフ");

    // Remove vehicle
    let removed = store.remove_vehicle(&id).expect("Failed to remove");
    assert!(removed);
    assert_eq!(store.count(), 1);
    assert!(store.get_vehicle(&id).is_none());

    println!("=== Vehicle Store Test Passed ===");
}
