/// Aura command handlers

use anyhow::Result;
use colored::*;
use crate::cli::AuraCommands;
use crate::routing::route_to_commander;

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
    }
}
