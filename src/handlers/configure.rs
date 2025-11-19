use crate::cli::ConfigureCommands;
use crate::routing::route_to_commander;
/// Configure command handlers
use anyhow::Result;
use colored::*;

pub fn handle_configure(command: ConfigureCommands) -> Result<()> {
    match command {
        ConfigureCommands::Show => {
            println!("{}", "⚙️  Configuration".cyan().bold());
            route_to_commander("configure", &["show"])
        }
        ConfigureCommands::Set { key, value } => {
            println!(
                "{}",
                format!("✏️  Setting {} = {}...", key, value).cyan().bold()
            );
            route_to_commander("configure", &["set", &key, &value])
        }
        ConfigureCommands::Reset => {
            println!("{}", "🔄 Resetting configuration...".yellow().bold());
            route_to_commander("configure", &["reset"])
        }
    }
}
