/// Claude command handlers

use anyhow::Result;
use colored::*;
use crate::cli::{ClaudeCommands, ProjectActions, SessionActions};
use crate::routing::route_to_commander;

pub fn handle_claude(command: ClaudeCommands) -> Result<()> {
    match command {
        ClaudeCommands::Session { action } => match action {
            SessionActions::List { limit } => {
                println!("{}", "📋 Listing Claude sessions...".cyan().bold());
                route_to_commander("claude", &["session", "list", "--limit", &limit.to_string()])
            }
            SessionActions::Recover { uuid } => {
                println!("{}", format!("🔄 Recovering session {}...", uuid).cyan().bold());
                route_to_commander("claude", &["session", "recover", &uuid])
            }
            SessionActions::View { uuid } => {
                println!("{}", format!("👁  Viewing session {}...", uuid).cyan().bold());
                route_to_commander("claude", &["session", "view", &uuid])
            }
        },
        ClaudeCommands::Project { action } => match action {
            ProjectActions::List => {
                println!("{}", "📂 Listing Claude projects...".cyan().bold());
                route_to_commander("claude", &["project", "list"])
            }
            ProjectActions::Migrate { path } => {
                println!("{}", format!("📦 Migrating project to {}...", path).cyan().bold());
                route_to_commander("claude", &["project", "migrate", &path])
            }
        },
    }
}
