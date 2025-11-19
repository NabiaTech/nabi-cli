use crate::cli::AuraCommands;
use crate::routing::route_to_commander;
/// Aura command handlers
// AURA Switch implementation - Phase C completion
use anyhow::Result;
use colored::*;

pub fn handle_aura(command: AuraCommands) -> Result<()> {
    match command {
        AuraCommands::List => {
            println!("{}", "📋 Listing AURAs...".cyan().bold());
            route_to_commander("aura", &["list"])
        }
        AuraCommands::Show { name } => {
            println!("{}", format!("👁  Viewing AURA: {}...", name).cyan().bold());
            route_to_commander("aura", &["show", &name])
        }
        AuraCommands::Create { name } => {
            println!("{}", format!("✨ Creating AURA: {}...", name).cyan().bold());
            route_to_commander("aura", &["create", &name])
        }
        AuraCommands::Switch { name, force } => {
            println!(
                "{}",
                format!("🔄 Switching to AURA: {}...", name).cyan().bold()
            );
            let mut args = vec!["switch", &name];
            let force_str = String::from("--force");
            if force {
                args.push(&force_str);
            }
            route_to_commander("aura", &args)
        }
        AuraCommands::Status => {
            println!("{}", "📊 Checking active AURA...".cyan().bold());
            route_to_commander("aura", &["status"])
        }
    }
}
