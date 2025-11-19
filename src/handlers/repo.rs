use crate::cli::{GraphActions, RepoCommands};
use crate::repo;
/// Repo command handlers
use anyhow::Result;

fn handle_graph(action: GraphActions) -> Result<()> {
    match action {
        GraphActions::Search {
            symbol,
            repo,
            format,
        } => {
            let repo_path = repo.unwrap_or_else(|| ".".to_string());
            repo::graph_search(&repo_path, &symbol, &format)
        }
        GraphActions::References {
            symbol,
            repo,
            format,
        } => {
            let repo_path = repo.unwrap_or_else(|| ".".to_string());
            repo::graph_references(&repo_path, &symbol, &format)
        }
        GraphActions::Related {
            symbol,
            repo,
            depth,
            format,
        } => {
            let repo_path = repo.unwrap_or_else(|| ".".to_string());
            repo::graph_related(&repo_path, &symbol, depth, &format)
        }
    }
}

pub fn handle_repo(command: RepoCommands) -> Result<()> {
    match command {
        RepoCommands::Check {
            path,
            format,
            strict,
        } => {
            let repo_path = path.unwrap_or_else(|| ".".to_string());
            repo::check(&repo_path, &format, strict)
        }
        RepoCommands::Analyze {
            repo_path,
            lang,
            force,
            format,
        } => repo::analyze(&repo_path, lang.as_deref(), force, &format),
        RepoCommands::Graph { action } => handle_graph(action),
        RepoCommands::Codegraph { command: _ } => {
            // Codegraph commands are handled via cli.rs routing, not here
            anyhow::bail!("Codegraph command should be handled through handlers")
        }
    }
}
