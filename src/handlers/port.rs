/// Port command handlers

use anyhow::Result;
use crate::cli::PortCommands;
use crate::commands::port as port_cmd;

pub fn handle_port(command: PortCommands) -> Result<()> {
    match command {
        PortCommands::List { platform } => {
            port_cmd::cmd_list(platform.as_deref())
        }
        PortCommands::Check => {
            port_cmd::cmd_check()
        }
        PortCommands::CrossPlatform => {
            port_cmd::cmd_cross_platform()
        }
        PortCommands::Shift { service, old_port, new_port, dry_run } => {
            port_cmd::cmd_shift(&service, old_port, new_port, dry_run)
        }
        PortCommands::Drift { forensic, since } => {
            port_cmd::cmd_drift(forensic, since.as_deref())
        }
        PortCommands::Fix => {
            port_cmd::cmd_fix()
        }
        PortCommands::GenerateEnv => {
            port_cmd::cmd_generate_env()
        }
    }
}
