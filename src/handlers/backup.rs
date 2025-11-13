/// Backup command handlers

use anyhow::Result;
use crate::cli::BackupCommands;
use crate::commands::backup as backup_cmd;

pub fn handle_backup(command: BackupCommands) -> Result<()> {
    match command {
        BackupCommands::Create {
            mode,
            dry_run,
            targets,
            external,
        } => {
            backup_cmd::cmd_create(
                mode.as_deref(),
                dry_run,
                targets.as_deref(),
                external,
            )
        }
        BackupCommands::List { format } => {
            backup_cmd::cmd_list(format.as_deref())
        }
        BackupCommands::Restore {
            backup_id,
            target,
            dry_run,
        } => {
            backup_cmd::cmd_restore(&backup_id, target.as_deref(), dry_run)
        }
        BackupCommands::Config { validate } => {
            backup_cmd::cmd_config(validate)
        }
        BackupCommands::Queue { action } => {
            backup_cmd::cmd_queue(action.as_deref())
        }
    }
}
