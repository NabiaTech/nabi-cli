use crate::cli::{ConfigActions, RecordCommands, ServerActions};
use crate::routing::route_to_commander;
/// Record command handlers
use anyhow::Result;
use colored::*;

pub fn handle_record(command: RecordCommands) -> Result<()> {
    match command {
        RecordCommands::Start { output } => {
            println!("{}", "🎬 Starting recording...".cyan().bold());
            if let Some(o) = output {
                route_to_commander("record", &["start", "--output", &o])
            } else {
                route_to_commander("record", &["start"])
            }
        }
        RecordCommands::Stop { id } => {
            println!("{}", "⏹️  Stopping recording...".cyan().bold());
            if let Some(i) = id {
                route_to_commander("record", &["stop", &i])
            } else {
                route_to_commander("record", &["stop"])
            }
        }
        RecordCommands::List => {
            println!("{}", "📋 Listing recordings...".cyan().bold());
            route_to_commander("record", &["list"])
        }
        RecordCommands::Server { action } => match action {
            ServerActions::Start => {
                println!("{}", "🚀 Starting server...".green().bold());
                route_to_commander("record", &["server", "start"])
            }
            ServerActions::Stop => {
                println!("{}", "⏹️  Stopping server...".yellow().bold());
                route_to_commander("record", &["server", "stop"])
            }
            ServerActions::Status => {
                println!("{}", "📊 Server status...".cyan().bold());
                route_to_commander("record", &["server", "status"])
            }
        },
        RecordCommands::Config { action } => match action {
            ConfigActions::Show => {
                println!("{}", "⚙️  Configuration...".cyan().bold());
                route_to_commander("record", &["config", "show"])
            }
            ConfigActions::Set { key, value } => {
                println!(
                    "{}",
                    format!("✏️  Setting {} = {}...", key, value).cyan().bold()
                );
                route_to_commander("record", &["config", "set", &key, &value])
            }
        },
    }
}
