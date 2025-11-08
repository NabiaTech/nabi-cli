/// Forge command handlers

use anyhow::Result;
use crate::cli::ForgeCommands;
use crate::forge;

pub fn handle_forge(command: ForgeCommands) -> Result<()> {
    match command {
        ForgeCommands::Enable { feature } => {
            forge::handle_enable(feature)
        }
        ForgeCommands::Disable { feature } => {
            forge::handle_disable(feature)
        }
        ForgeCommands::Status => {
            forge::handle_status()
        }
        ForgeCommands::List => {
            forge::handle_list()
        }
    }
}
