// Main entry point for gstats - Git Repository Analytics Tool

mod cli;
mod git;

use anyhow::Result;
use std::process;

fn main() {
    if let Err(e) = run() {
        eprintln!("Error: {}", e);
        process::exit(1);
    }
}

fn run() -> Result<()> {
    let args = cli::parse_args();
    
    // Validate git repository
    let repo_path = git::resolve_repository_path(args.repository)?;
    
    println!("Analyzing git repository at: {}", repo_path);
    
    Ok(())
}
