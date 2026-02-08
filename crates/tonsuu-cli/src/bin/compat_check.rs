use std::io::Write;
use std::path::PathBuf;

use clap::Parser;
use tonsuu_app::config::Config;
use tonsuu_app::repository::{open_history_store_at, open_vehicle_store_at};
use tonsuu_infra::overload_csv::{load_slips_from_csv, load_vehicles_from_csv};
use serde::Serialize;

#[derive(Parser, Debug)]
#[command(name = "compat_check", about = "Compatibility check for tonsuu-checker data")]
struct Args {
    /// Optional config file path (defaults to app config location)
    #[arg(long)]
    config: Option<PathBuf>,

    /// Store directory override
    #[arg(long)]
    store_dir: Option<PathBuf>,

    /// Path to weighing slips CSV for overload check
    #[arg(long)]
    slips_csv: Option<PathBuf>,

    /// Path to vehicle master CSV for overload check
    #[arg(long)]
    vehicles_csv: Option<PathBuf>,

    /// Write JSONL output to file
    #[arg(long)]
    jsonl: Option<PathBuf>,

    /// Write pretty JSON summary to file
    #[arg(long)]
    json: Option<PathBuf>,
}

#[derive(Debug, Serialize)]
struct Summary {
    store_dir: String,
    history_count: usize,
    feedback_count: usize,
    vehicle_count: usize,
    slips_count: Option<usize>,
    vehicles_master_count: Option<usize>,
}

fn main() {
    let args = Args::parse();

    let config = match args.config {
        Some(path) => {
            match std::fs::read_to_string(&path) {
                Ok(content) => serde_json::from_str::<Config>(&content).unwrap_or_default(),
                Err(e) => {
                    eprintln!("Failed to read config at {}: {}", path.display(), e);
                    std::process::exit(1);
                }
            }
        }
        None => Config::load().unwrap_or_default(),
    };

    let store_dir = match args.store_dir {
        Some(dir) => dir,
        None => config.store_dir().unwrap_or_else(|_| std::env::temp_dir().join("tonsuu-checker")),
    };

    println!("[Store] Dir: {}", store_dir.display());

    let mut summary = Summary {
        store_dir: store_dir.display().to_string(),
        history_count: 0,
        feedback_count: 0,
        vehicle_count: 0,
        slips_count: None,
        vehicles_master_count: None,
    };

    let store = match open_history_store_at(store_dir.clone()) {
        Ok(store) => store,
        Err(e) => {
            eprintln!("[Store] failed to open: {}", e);
            std::process::exit(1);
        }
    };
    summary.history_count = store.count();
    summary.feedback_count = store.feedback_count();
    println!("[Store] history.json entries: {}", summary.history_count);
    println!("[Store] feedback entries: {}", summary.feedback_count);

    let vehicles = match open_vehicle_store_at(store_dir.clone()) {
        Ok(vehicles) => vehicles,
        Err(e) => {
            eprintln!("[VehicleStore] failed to open: {}", e);
            std::process::exit(1);
        }
    };
    summary.vehicle_count = vehicles.count();
    println!("[VehicleStore] vehicles.json entries: {}", summary.vehicle_count);

    if let (Some(slips_csv), Some(vehicles_csv)) = (args.slips_csv, args.vehicles_csv) {
        println!("[OverloadCSV] slips: {}", slips_csv.display());
        println!("[OverloadCSV] vehicles: {}", vehicles_csv.display());

        match load_slips_from_csv(&slips_csv) {
            Ok(slips) => {
                summary.slips_count = Some(slips.len());
                println!("[OverloadCSV] slips loaded: {}", slips.len());
            }
            Err(e) => {
                eprintln!("[OverloadCSV] slips load failed: {}", e);
                std::process::exit(1);
            }
        }

        match load_vehicles_from_csv(&vehicles_csv) {
            Ok(vehicles) => {
                summary.vehicles_master_count = Some(vehicles.len());
                println!("[OverloadCSV] vehicles loaded: {}", vehicles.len());
            }
            Err(e) => {
                eprintln!("[OverloadCSV] vehicles load failed: {}", e);
                std::process::exit(1);
            }
        }
    } else {
        println!("[OverloadCSV] skipped (provide --slips-csv and --vehicles-csv)");
    }

    if let Some(path) = args.jsonl {
        if let Ok(mut file) = std::fs::File::create(&path) {
            let _ = writeln!(
                file,
                "{}",
                serde_json::json!({
                    "event": "store",
                    "store_dir": summary.store_dir,
                    "history_count": summary.history_count,
                    "feedback_count": summary.feedback_count,
                    "vehicle_count": summary.vehicle_count
                })
            );
            let _ = writeln!(
                file,
                "{}",
                serde_json::json!({
                    "event": "overload_csv",
                    "slips_count": summary.slips_count,
                    "vehicles_master_count": summary.vehicles_master_count
                })
            );
        } else {
            eprintln!("[JSONL] failed to write: {}", path.display());
        }
    }

    if let Some(path) = args.json {
        if let Ok(content) = serde_json::to_string_pretty(&summary) {
            if let Err(e) = std::fs::write(&path, content) {
                eprintln!("[JSON] failed to write: {}", e);
            }
        }
    }
}
