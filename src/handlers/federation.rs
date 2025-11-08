/// Federation command handlers

use anyhow::Result;
use colored::*;
use crate::cli::{FederationCommands, AgentActions, SyncActions, RegistryActions};
use crate::routing::route_to_commander;

pub fn handle_federation(command: FederationCommands) -> Result<()> {
    match command {
        FederationCommands::Agent { action } => match action {
            AgentActions::List => {
                println!("{}", "🤖 Listing federation agents...".magenta().bold());
                route_to_commander("federation", &["agent", "list"])
            }
            AgentActions::Spawn { role } => {
                println!("{}", format!("🚀 Spawning {} agent...", role).magenta().bold());
                route_to_commander("federation", &["agent", "spawn", &role])
            }
        },
        FederationCommands::Sync { action } => match action {
            SyncActions::List => {
                println!("{}", "📂 Listing Syncthing folders...".cyan().bold());
                route_to_commander("federation", &["sync", "list"])
            }
            SyncActions::Pause { folder } => {
                println!("{}", format!("⏸️  Pausing folder {}...", folder).yellow().bold());
                route_to_commander("federation", &["sync", "pause", &folder])
            }
            SyncActions::Resume { folder } => {
                println!("{}", format!("▶️  Resuming folder {}...", folder).green().bold());
                route_to_commander("federation", &["sync", "resume", &folder])
            }
            SyncActions::Status { folder } => {
                if let Some(ref f) = folder {
                    println!("{}", format!("📊 Status for {}...", f).cyan().bold());
                    route_to_commander("federation", &["sync", "status", f])
                } else {
                    println!("{}", "📊 Status for all folders...".cyan().bold());
                    route_to_commander("federation", &["sync", "status"])
                }
            }
        },
        FederationCommands::Registry { action } => match action {
            RegistryActions::List => {
                println!("{}", "📋 Listing registered services...".blue().bold());
                route_to_commander("federation", &["registry", "list"])
            }
            RegistryActions::Health => {
                println!("{}", "🏥 Checking service health...".blue().bold());
                route_to_commander("federation", &["registry", "health"])
            }
            RegistryActions::Add { name, service_type } => {
                println!("{}", format!("➕ Adding {} ({})...", name, service_type).green().bold());
                route_to_commander("federation", &["registry", "add", &name, "--type", &service_type])
            }
            RegistryActions::Remove { name } => {
                println!("{}", format!("➖ Removing {}...", name).red().bold());
                route_to_commander("federation", &["registry", "remove", &name])
            }
        },
        FederationCommands::Health => {
            println!("{}", "🩺 Running federation health checks...".cyan().bold());
            // For now, this is a placeholder.
            // In the future, this will call the federation_health.py script or similar.
            println!("  - Loki status: {}", "Pending".yellow());
            println!("  - NATS status: {}", "Pending".yellow());
            println!("  - Tmux status: {}", "Pending".yellow());
            Ok(())
        }
        FederationCommands::Status => {
            println!("{}", "📊 Federation status...".cyan().bold());
            println!("  - Node discovery: {}", "Pending".yellow());
            Ok(())
        }
        FederationCommands::Agents => {
            println!("{}", "🤖 Listing all active agents...".cyan().bold());
            println!("  - Agent query: {}", "Pending".yellow());
            Ok(())
        }
    }
}
