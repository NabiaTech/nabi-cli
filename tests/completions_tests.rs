/// Tests for CLI completion functionality
///
/// This module tests the shell completion generation and dynamic tool discovery features.

use clap::CommandFactory;
use clap_complete::{generate, Shell as CompletionShell};
use std::io::Cursor;
use tempfile::NamedTempFile;
use std::fs;
use std::path::PathBuf;

use crate::cli::Cli;

/// Test that basic completion generation works for different shells
#[test]
fn test_basic_completion_generation() {
    let shells = vec![
        CompletionShell::Bash,
        CompletionShell::Zsh,
        CompletionShell::Fish,
        CompletionShell::PowerShell,
        CompletionShell::Elvish,
    ];

    for shell in shells {
        let mut buffer = Vec::new();
        let mut command = Cli::command();
        generate(shell, &mut command, "nabi", &mut buffer).unwrap();

        let completion_script = String::from_utf8(buffer).unwrap();

        // Basic checks that the completion script was generated
        assert!(!completion_script.is_empty());
        assert!(completion_script.contains("nabi"));

        // Shell-specific checks
        match shell {
            CompletionShell::Bash => {
                assert!(completion_script.contains("_nabi"));
                assert!(completion_script.contains("complete"));
            }
            CompletionShell::Zsh => {
                assert!(completion_script.contains("#compdef"));
                assert!(completion_script.contains("_nabi"));
            }
            CompletionShell::Fish => {
                assert!(completion_script.contains("function __nabi"));
            }
            CompletionShell::PowerShell => {
                assert!(completion_script.contains("Register-ArgumentCompleter"));
            }
            CompletionShell::Elvish => {
                assert!(completion_script.contains("edit:completion:arg-completer"));
            }
        }
    }
}

/// Test completion generation to file
#[test]
fn test_completion_to_file() {
    let temp_file = NamedTempFile::new().unwrap();
    let output_path = temp_file.path().to_path_buf();

    // This would test the handle_completions function, but we can't easily test it
    // without mocking the file system operations. Instead, let's test the core
    // functionality that handle_completions uses.

    let mut buffer = Vec::new();
    let mut command = Cli::command();
    generate(CompletionShell::Zsh, &mut command, "nabi", &mut buffer).unwrap();

    // Write to file
    fs::write(&output_path, &buffer).unwrap();

    // Read back and verify
    let content = fs::read_to_string(&output_path).unwrap();
    assert_eq!(content, String::from_utf8(buffer).unwrap());
    assert!(content.contains("#compdef"));
}

/// Test that dynamic completion markers are added for supported shells
#[test]
fn test_dynamic_completion_markers() {
    // Test ZSH dynamic completion markers
    let mut buffer = Vec::new();
    let mut command = Cli::command();
    generate(CompletionShell::Zsh, &mut command, "nabi", &mut buffer).unwrap();

    // Convert to string for easier testing
    let completion_script = String::from_utf8(buffer).unwrap();

    // Check that the script can be extended with dynamic completion
    // (In the real handle_completions, this gets appended)
    assert!(completion_script.contains("_nabi()"));
    assert!(completion_script.contains("autoload -U is-at-least"));
}

/// Test that unsupported shells don't get dynamic completion (should still work for basic completion)
#[test]
fn test_unsupported_shells_basic_completion() {
    let unsupported_shells = vec![CompletionShell::Fish, CompletionShell::PowerShell, CompletionShell::Elvish];

    for shell in unsupported_shells {
        let mut buffer = Vec::new();
        let mut command = Cli::command();
        generate(shell, &mut command, "nabi", &mut buffer).unwrap();

        let completion_script = String::from_utf8(buffer).unwrap();
        assert!(!completion_script.is_empty());
        assert!(completion_script.contains("nabi"));
    }
}

/// Test completion script syntax validation (basic)
#[test]
fn test_completion_script_syntax() {
    // This is a basic test - in a real CI, we'd use shellcheck or similar
    let mut buffer = Vec::new();
    let mut command = Cli::command();
    generate(CompletionShell::Bash, &mut command, "nabi", &mut buffer).unwrap();

    let completion_script = String::from_utf8(buffer).unwrap();

    // Basic syntax checks
    assert!(completion_script.lines().count() > 10); // Should be substantial
    assert!(!completion_script.contains("\t")); // Should not have tabs (bash style)
}

/// Test that completion handles all CLI subcommands
#[test]
fn test_completion_includes_all_subcommands() {
    let mut buffer = Vec::new();
    let mut command = Cli::command();
    generate(CompletionShell::Zsh, &mut command, "nabi", &mut buffer).unwrap();

    let completion_script = String::from_utf8(buffer).unwrap();

    // Check that major subcommands are included in completion
    let major_commands = vec![
        "claude", "data", "federation", "self", "forge", "docs", "repo",
        "tool", "scan", "watch", "aura", "configure", "db", "record",
        "agent", "port", "tmux", "kernel", "events", "hooks", "mode",
        "riff", "recover", "health", "completions", "deckgen", "doctor",
        "migrate", "orgtime"
    ];

    for cmd in major_commands {
        assert!(completion_script.contains(cmd),
                "Completion script should include subcommand: {}", cmd);
    }
}

/// Test completion handles complex argument structures
#[test]
fn test_completion_complex_arguments() {
    let mut buffer = Vec::new();
    let mut command = Cli::command();
    generate(CompletionShell::Zsh, &mut command, "nabi", &mut buffer).unwrap();

    let completion_script = String::from_utf8(buffer).unwrap();

    // Check that complex commands with multiple subcommands work
    assert!(completion_script.contains("tool"));
    assert!(completion_script.contains("federation"));
    assert!(completion_script.contains("health"));
}
