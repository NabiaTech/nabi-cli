use crate::cli::SelfCommands;
use crate::paths::NabiPaths;
use crate::routing::{check_commander, update_commander};
use crate::spec::handle_spec;
/// Self-management command handlers
use anyhow::Result;
use colored::*;
use std::fs;
use std::process::Command;

fn check_xdg_compliance() -> Result<()> {
    let mut violations = Vec::new();

    // Check 1: No .venv artifacts in config directories
    let config_dir = NabiPaths::config_dir()?;
    let config_venv = config_dir.join(".venv");
    let nabi_venv = config_dir.join(".nabi").join(".venv");

    if config_venv.exists() {
        let config_venv_path = NabiPaths::config_dir()?.join(".venv");
        violations.push((
            format!(
                "Broken .venv in config directory: {}",
                config_venv.display()
            ),
            format!("rm -rf {}", config_venv_path.display()),
        ));
    }

    if nabi_venv.exists() {
        violations.push((
            format!(
                "Broken .venv in .nabi subdirectory: {}",
                nabi_venv.display()
            ),
            format!("rm -rf {}", nabi_venv.display()),
        ));
    }

    if violations.is_empty() {
        println!("  {} XDG compliance", "✓".green());
        Ok(())
    } else {
        println!("  {} XDG compliance violations found:", "✗".red());
        for (issue, fix) in violations {
            println!("    - {}", issue);
            println!("      Fix: {}", fix);
        }
        Ok(())
    }
}

fn handle_diagnose(quick: bool, aura: bool, format: &str) -> Result<()> {
    if format == "json" {
        // JSON output mode
        let mut data = serde_json::json!({
            "nabi_function": null,
            "nabi_binary": null,
            "aura": null,
        });

        // Check nabi function
        let type_output = Command::new("sh")
            .arg("-c")
            .arg("type nabi 2>&1 | head -3")
            .output()?;
        data["nabi_function"] = serde_json::json!(String::from_utf8_lossy(&type_output.stdout).to_string());

        // Check nabi binary
        let which_output = Command::new("which").arg("nabi").output()?;
        if which_output.status.success() {
            data["nabi_binary"] = serde_json::json!(String::from_utf8_lossy(&which_output.stdout).trim());
        }

        // Check aura
        let aura_file = std::path::PathBuf::from(std::env::var("HOME")?)
            .join(".local/state/nabi/active_aura.json");
        if aura_file.exists() {
            if let Ok(contents) = fs::read_to_string(&aura_file) {
                data["aura"] = serde_json::from_str(&contents).unwrap_or(serde_json::json!(contents));
            }
        }

        println!("{}", serde_json::to_string_pretty(&data)?);
        return Ok(());
    }

    // Text output mode
    println!("{}", "🔍 Nabi Shell Diagnostics".blue().bold());
    println!("{}", "━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━".cyan());
    println!();

    // 1. Nabi function definition
    println!("{}", "1. Nabi function definition:".yellow().bold());
    let type_output = Command::new("sh")
        .arg("-c")
        .arg("type nabi 2>&1 | head -3")
        .output()?;
    print!("{}", String::from_utf8_lossy(&type_output.stdout));
    println!();

    // 2. Nabi binary availability
    println!("{}", "2. Nabi binary availability:".yellow().bold());
    match Command::new("which").arg("nabi").output() {
        Ok(output) if output.status.success() => {
            println!("  {} {}", "✓".green(), String::from_utf8_lossy(&output.stdout).trim());
        }
        _ => {
            println!("  {} nabi binary not found in PATH", "✗".red());
        }
    }
    println!();

    // 3. System health check (unless --quick)
    if !quick {
        println!("{}", "3. Running system health check:".yellow().bold());
        handle_self(SelfCommands::Doctor)?;
        println!();
    }

    // 4. Active aura
    println!("{}", "4. Active aura:".yellow().bold());
    let aura_file = std::path::PathBuf::from(std::env::var("HOME")?)
        .join(".local/state/nabi/active_aura.json");

    if aura_file.exists() {
        if let Ok(contents) = fs::read_to_string(&aura_file) {
            if aura {
                // Show full aura details
                println!("{}", contents);
            } else {
                // Extract just the aura name
                if let Ok(json) = serde_json::from_str::<serde_json::Value>(&contents) {
                    if let Some(aura_name) = json.get("aura").and_then(|v| v.as_str()) {
                        println!("  {} Active: {}", "✓".green(), aura_name.cyan());
                    } else {
                        println!("  {} Aura file exists but unreadable", "⚠".yellow());
                    }
                }
            }
        }
    } else {
        println!("  {} Aura state not found. Available auras:", "⚠".yellow());
        let auras_dir = std::path::PathBuf::from(std::env::var("HOME")?)
            .join(".config/nabi/auras");
        if let Ok(entries) = fs::read_dir(auras_dir) {
            for entry in entries.flatten() {
                if let Some(name) = entry.path().file_stem() {
                    println!("     - {}", name.to_string_lossy().cyan());
                }
            }
        }
        println!();
        println!("  To activate an aura: {}", "nabi aura switch <aura_name>".cyan());
    }

    Ok(())
}

pub fn handle_self(command: SelfCommands) -> Result<()> {
    match command {
        SelfCommands::Doctor => {
            println!("{}", "🏥 Running health check...".blue().bold());
            check_commander("claude")?;
            check_commander("data")?;
            check_commander("federation")?;
            check_xdg_compliance()?;
            println!("{}", "✓ All commanders healthy!".green().bold());
            Ok(())
        }
        SelfCommands::Update => {
            println!("{}", "⬆️  Updating all commanders...".blue().bold());
            update_commander("claude")?;
            update_commander("data")?;
            update_commander("federation")?;
            println!("{}", "✓ All commanders updated!".green().bold());
            Ok(())
        }
        SelfCommands::Config => {
            println!("{}", "⚙️  Configuration".blue().bold());
            let config_dir = NabiPaths::config_dir()?;
            let data_dir = NabiPaths::data_dir()?;
            let cache_dir = NabiPaths::cache_dir()?;
            println!("  Config dir: {}", config_dir.display());
            println!("  Data dir:   {}", data_dir.display());
            println!("  Cache dir:  {}", cache_dir.display());
            Ok(())
        }
        SelfCommands::Spec { format } => handle_spec(format),
        SelfCommands::Diagnose { quick, aura, format } => {
            handle_diagnose(quick, aura, &format)
        }
    }
}
