use crate::cli::{DataCommands, JsonlActions};
use crate::routing::route_to_commander;
/// Data command handlers
use anyhow::Result;
use colored::*;

pub fn handle_data(command: DataCommands) -> Result<()> {
    match command {
        DataCommands::Jsonl { action } => match action {
            JsonlActions::Validate { file } => {
                println!("{}", format!("✓ Validating {}...", file).green().bold());
                route_to_commander("data", &["jsonl", "validate", &file])
            }
            JsonlActions::Repair { file, output } => {
                let output_file = output.unwrap_or_else(|| format!("{}.fixed", file));
                println!(
                    "{}",
                    format!("🔧 Repairing {} -> {}...", file, output_file)
                        .yellow()
                        .bold()
                );
                route_to_commander(
                    "data",
                    &["jsonl", "repair", &file, "--output", &output_file],
                )
            }
            JsonlActions::View { file, query } => {
                println!("{}", format!("👁  Viewing {}...", file).cyan().bold());
                let mut args = vec!["jsonl", "view", &file];
                let query_str;
                if let Some(ref q) = query {
                    query_str = q.clone();
                    args.extend_from_slice(&["--query", &query_str]);
                }
                route_to_commander("data", &args)
            }
        },
    }
}
