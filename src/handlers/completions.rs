/// Completions handler

use anyhow::Result;
use clap::{CommandFactory};
use clap_complete::{generate, Shell as CompletionShell};
use std::io;
use crate::cli::Cli;

pub fn handle_completions(shell: CompletionShell) -> Result<()> {
    let mut command = Cli::command();
    generate(shell, &mut command, "nabi", &mut io::stdout());
    Ok(())
}
