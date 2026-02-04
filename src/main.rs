//! Tonnage Checker - Dump truck cargo weight estimation using AI
//!
//! A CLI tool that analyzes images of dump trucks to estimate cargo weight.

mod analyzer;
mod app;
mod cli;
mod commands;
mod config;
mod constants;
mod domain;
mod error;
mod export;
mod infrastructure;
mod output;
mod scanner;
mod store;
mod types;
mod vision;

/// Re-export plate_local for backwards compatibility
mod plate_local {
    
}

use clap::Parser;
use cli::Cli;

fn main() {
    let cli = Cli::parse();

    if let Err(e) = commands::execute(cli) {
        eprintln!("Error: {}", e);
        std::process::exit(1);
    }
}
