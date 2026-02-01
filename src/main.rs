//! Tonnage Checker - Dump truck cargo weight estimation using AI
//!
//! A CLI tool that analyzes images of dump trucks to estimate cargo weight.

mod analyzer;
mod cli;
mod commands;
mod config;
mod constants;
mod error;
mod export;
mod scanner;
mod types;

use clap::Parser;
use cli::Cli;

fn main() {
    let cli = Cli::parse();

    if let Err(e) = commands::execute(cli) {
        eprintln!("Error: {}", e);
        std::process::exit(1);
    }
}
