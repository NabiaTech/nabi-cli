// ASCII Diagram Validator - nabi-cli integration
// Provides subcommands for validating and fixing ASCII diagrams in markdown/text files

use anyhow::{Context, Result};
use clap::Subcommand;
use std::path::PathBuf;
use std::process::{Command, Stdio};

#[derive(Subcommand)]
pub enum AsciiCommands {
    /// Validate ASCII diagrams in a file (read-only check)
    Validate {
        /// Path to markdown or text file with ASCII diagrams
        #[arg(value_name = "FILE")]
        file: PathBuf,

        /// Show detailed validation report
        #[arg(short, long)]
        verbose: bool,

        /// Output results as JSON
        #[arg(short, long)]
        json: bool,
    },

    /// Auto-fix ASCII diagram alignment issues
    Fix {
        /// Path to markdown or text file with ASCII diagrams
        #[arg(value_name = "FILE")]
        file: PathBuf,

        /// Show detailed validation report after fixing
        #[arg(short, long)]
        report: bool,

        /// Show detailed output
        #[arg(short, long)]
        verbose: bool,
    },

    /// Quick check with automatic fixing
    Check {
        /// Path to markdown or text file with ASCII diagrams
        #[arg(value_name = "FILE")]
        file: PathBuf,

        /// Show detailed output
        #[arg(short, long)]
        verbose: bool,

        /// Output results as JSON
        #[arg(short, long)]
        json: bool,
    },

    /// Show detailed validation report
    Report {
        /// Path to markdown or text file with ASCII diagrams
        #[arg(value_name = "FILE")]
        file: PathBuf,
    },
}

/// Handle ASCII diagram validator commands
pub fn handle_ascii_commands(command: AsciiCommands) -> Result<()> {
    match command {
        AsciiCommands::Validate { file, verbose, json } => {
            handle_validate(&file, verbose, json)
        }
        AsciiCommands::Fix { file, report, verbose } => {
            handle_fix(&file, report, verbose)
        }
        AsciiCommands::Check { file, verbose, json } => {
            handle_check(&file, verbose, json)
        }
        AsciiCommands::Report { file } => {
            handle_report(&file)
        }
    }
}

fn handle_validate(file: &PathBuf, verbose: bool, json: bool) -> Result<()> {
    let cli_path = get_validator_cli_path()?;

    let mut args = vec!["--operation", "validate"];

    if verbose {
        args.push("--verbose");
    }

    if json {
        args.push("--json");
    }

    args.push(file.to_str().context("Invalid file path")?);

    run_validator(&cli_path, &args)
}

fn handle_fix(file: &PathBuf, report: bool, verbose: bool) -> Result<()> {
    let cli_path = get_validator_cli_path()?;

    let mut args = vec!["--operation", "fix"];

    if report {
        args.push("--operation");
        args.push("report");
    }

    if verbose {
        args.push("--verbose");
    }

    args.push(file.to_str().context("Invalid file path")?);

    run_validator(&cli_path, &args)
}

fn handle_check(file: &PathBuf, verbose: bool, json: bool) -> Result<()> {
    let cli_path = get_validator_cli_path()?;

    let mut args = vec!["--operation", "check"];

    if verbose {
        args.push("--verbose");
    }

    if json {
        args.push("--json");
    }

    args.push(file.to_str().context("Invalid file path")?);

    run_validator(&cli_path, &args)
}

fn handle_report(file: &PathBuf) -> Result<()> {
    let cli_path = get_validator_cli_path()?;

    let args = vec!["--operation", "report", file.to_str().context("Invalid file path")?];

    run_validator(&cli_path, &args)
}

fn get_validator_cli_path() -> Result<PathBuf> {
    // Try to find the validator CLI in the expected location
    // It should be at ~/nabia/core/hooks/src/validator/cli.py
    let home = std::env::var("HOME")
        .context("HOME environment variable not set")?;

    let validator_path = PathBuf::from(home)
        .join("nabia/core/hooks/src/validator/cli.py");

    if !validator_path.exists() {
        return Err(anyhow::anyhow!(
            "ASCII diagram validator not found at {}. \
            Ensure ~/nabia/core/hooks/src/validator/cli.py is available.",
            validator_path.display()
        ));
    }

    Ok(validator_path)
}

fn run_validator(cli_path: &PathBuf, args: &[&str]) -> Result<()> {
    let output = Command::new("python3")
        .arg(cli_path)
        .args(args)
        .stdout(Stdio::inherit())
        .stderr(Stdio::inherit())
        .output()
        .context("Failed to execute ASCII diagram validator")?;

    if !output.status.success() {
        std::process::exit(output.status.code().unwrap_or(1));
    }

    Ok(())
}
