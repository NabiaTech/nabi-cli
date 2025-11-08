/// Database command handlers

use anyhow::Result;
use colored::*;
use crate::cli::DbCommands;
use crate::routing::route_to_commander;

pub fn handle_db(command: DbCommands) -> Result<()> {
    match command {
        DbCommands::Init => {
            println!("{}", "🗄️  Initializing database...".blue().bold());
            route_to_commander("db", &["init"])
        }
        DbCommands::Export { path } => {
            println!("{}", format!("💾 Exporting database to {}...", path).blue().bold());
            route_to_commander("db", &["export", &path])
        }
        DbCommands::Import { path } => {
            println!("{}", format!("📥 Importing database from {}...", path).blue().bold());
            route_to_commander("db", &["import", &path])
        }
    }
}
