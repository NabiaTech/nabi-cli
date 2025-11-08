/// Self-management command handlers

use anyhow::Result;
use colored::*;
use crate::cli::{SelfCommands, SpecFormat};
use crate::paths::NabiPaths;
use crate::routing::{check_commander, update_commander};
use crate::spec::handle_spec;

fn check_xdg_compliance() -> Result<()> {
    let mut violations = Vec::new();

    // Check 1: No .venv artifacts in config directories
    let config_dir = NabiPaths::config_dir()?;
    let config_venv = config_dir.join(".venv");
    let nabi_venv = config_dir.join(".nabi").join(".venv");

    if config_venv.exists() {
        let config_venv_path = NabiPaths::config_dir()?.join(".venv");
        violations.push((
            format!("Broken .venv in config directory: {}", config_venv.display()),
            format!("rm -rf {}", config_venv_path.display())
        ));
    }

    if nabi_venv.exists() {
        violations.push((
            format!("Broken .venv in .nabi subdirectory: {}", nabi_venv.display()),
            format!("rm -rf {}", nabi_venv.display())
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
    }
}
