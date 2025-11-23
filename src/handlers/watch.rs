use crate::cli::WatchCommands;
use crate::routing::route_to_commander;
/// Watch command handler
use anyhow::Result;
use colored::*;
use std::process::Command;

pub fn handle_watch(command: Option<WatchCommands>) -> Result<()> {
    match command {
        Some(WatchCommands::Events) => {
            println!("{}", "👁  Watching federation events...".cyan().bold());
            let status = Command::new("watch-events").status()?;
            if !status.success() {
                anyhow::bail!("watch-events failed");
            }
            Ok(())
        }
        Some(WatchCommands::Federation) => {
            println!("{}", "👁  Watching federation coordination...".cyan().bold());
            let status = Command::new("fed-live-watch").status()?;
            if !status.success() {
                anyhow::bail!("fed-live-watch failed");
            }
            Ok(())
        }
        Some(WatchCommands::Tmp) => {
            println!("{}", "👁  Watching tmp directory...".cyan().bold());
            let status = Command::new("nabi-tmp-watcher").status()?;
            if !status.success() {
                anyhow::bail!("nabi-tmp-watcher failed");
            }
            Ok(())
        }
        Some(WatchCommands::Schema) => {
            println!("{}", "👁  Watching schema changes...".cyan().bold());
            let status = Command::new("schema-watch.sh").status()?;
            if !status.success() {
                anyhow::bail!("schema-watch.sh failed");
            }
            Ok(())
        }
        Some(WatchCommands::Path { path }) => {
            println!("{}", "👁  Watching filesystem...".cyan().bold());
            route_to_commander("watch", &["watch", &path])
        }
        None => {
            // Default: watch current directory
            println!("{}", "👁  Watching current directory...".cyan().bold());
            route_to_commander("watch", &["watch", "."])
        }
    }
}
