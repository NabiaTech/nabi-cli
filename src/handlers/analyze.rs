use crate::repo;
/// Analyze command handlers
use anyhow::Result;

// Note: This handler works with types from cli.rs
// For main.rs compatibility, we also have an inline handler
pub fn handle_analyze_from_cli(command: crate::cli::AnalyzeCommands) -> Result<()> {
    match command {
        crate::cli::AnalyzeCommands::Repo {
            repo_path,
            lang,
            force,
            format,
        } => repo::analyze(&repo_path, lang.as_deref(), force, &format),
    }
}
