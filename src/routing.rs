/// Commander routing logic
///
/// Handles routing commands to appropriate commanders (native Rust or Python fallback)

use anyhow::{Context, Result};
use colored::*;
use std::process;
use crate::paths::NabiPaths;

/// Route a command to the appropriate commander
pub fn route_to_commander(commander: &str, args: &[&str]) -> Result<()> {
    // Use XDG Base Directory spec across all platforms
    let nabi_config = NabiPaths::config_dir()?;

    let commander_path = nabi_config
        .join("commanders")
        .join(commander);

    // Check if a native Rust commander binary exists
    let commander_binary = commander_path.join(commander);
    if commander_binary.exists() {
        println!(
            "{}",
            format!("→ Route to {} commander (native)", commander)
                .dimmed()
        );

        let mut cmd = process::Command::new(&commander_binary);
        cmd.args(args);
        let status = cmd.status()
            .context(format!("Failed to execute commander at {}", commander_binary.display()))?;

        if !status.success() {
            process::exit(status.code().unwrap_or(1));
        }
        return Ok(());
    }

    // Fallback: Route to Python CLI for commands not yet migrated to Rust
    // This enables gradual migration: Python → Rust
    println!(
        "{}",
        format!("→ Route to Python CLI: {}", commander)
            .dimmed()
    );

    let bin_dir = NabiPaths::bin_dir()?;
    let python_cli = bin_dir.join("nabi-python");

    if python_cli.exists() {
        let mut cmd = process::Command::new(&python_cli);
        cmd.arg(commander);
        cmd.args(args);
        let status = cmd.status()
            .context(format!("Failed to execute Python CLI at {}", python_cli.display()))?;

        if !status.success() {
            process::exit(status.code().unwrap_or(1));
        }
        Ok(())
    } else {
        eprintln!(
            "{}",
            format!("❌ Commander '{}' not found and no Python CLI fallback available", commander)
                .red()
                .bold()
        );
        eprintln!("{}", format!("Expected Python CLI: {}", python_cli.display()).yellow());
        eprintln!("{}", "Run 'nabi self doctor' to diagnose issues.".yellow());
        process::exit(1);
    }
}

/// Check if a commander exists
pub fn check_commander(commander: &str) -> Result<()> {
    let nabi_config = NabiPaths::config_dir()?;

    let commander_path = nabi_config
        .join("commanders")
        .join(commander);

    if commander_path.exists() {
        println!("  {} {} {}", "✓".green(), commander, "present".dimmed());
        Ok(())
    } else {
        println!("  {} {} {}", "✗".red(), commander, "missing".dimmed());
        Ok(())
    }
}

/// Update a commander (placeholder for future implementation)
pub fn update_commander(commander: &str) -> Result<()> {
    println!("  {} Updating {}...", "→".blue(), commander);
    Ok(())
}
