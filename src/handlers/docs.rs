use crate::cli::{DocsCommands, ManifestActions};
use crate::routing::route_to_commander;
/// Docs command handlers
use anyhow::Result;
use colored::*;

pub fn handle_docs(command: DocsCommands) -> Result<()> {
    match command {
        DocsCommands::Manifest { action } => match action {
            ManifestActions::List => {
                println!("{}", "📄 Listing all manifests...".cyan().bold());
                route_to_commander("docs", &["manifest", "list"])
            }
            ManifestActions::Validate { repo_path } => {
                println!(
                    "{}",
                    format!("🔍 Validating manifest for {}...", repo_path)
                        .cyan()
                        .bold()
                );
                route_to_commander("docs", &["manifest", "validate", &repo_path])
            }
            ManifestActions::Generate { repo_path } => {
                println!(
                    "{}",
                    format!("✨ Generating manifest for {}...", repo_path)
                        .cyan()
                        .bold()
                );
                route_to_commander("docs", &["manifest", "generate", &repo_path])
            }
        },
    }
}
