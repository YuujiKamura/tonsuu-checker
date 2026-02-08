//! Tonnage Checker - Dump truck cargo weight estimation using AI
//!
//! A CLI tool that analyzes images of dump trucks to estimate cargo weight.

mod cli;
mod commands;
mod output;

use clap::Parser;
use cli::Cli;

fn main() {
    let cli = Cli::parse();

    if let Err(e) = commands::execute(cli) {
        eprintln!("Error: {}", e);
        std::process::exit(1);
    }
}
