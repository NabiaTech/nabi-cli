/// Integration tests for CLI completion functionality
///
/// These tests verify that the full completion pipeline works end-to-end,
/// including dynamic tool discovery and file operations.
use std::fs;
use std::path::PathBuf;
use std::process::Command;
use tempfile::TempDir;

/// Get the path to the built nabi binary
fn get_nabi_binary() -> PathBuf {
    // Use the XDG cache location where justfile builds to
    let xdg_cache = std::env::var("XDG_CACHE_HOME")
        .unwrap_or_else(|_| format!("{}/.cache", std::env::var("HOME").unwrap()));
    PathBuf::from(format!("{}/nabi/nabi-cli/target/release/nabi", xdg_cache))
}

/// Test the full completion pipeline by running the actual binary
/// This tests the integration of static + dynamic completions
#[test]
fn test_full_completion_pipeline_zsh() {
    let nabi_binary = get_nabi_binary();
    assert!(
        nabi_binary.exists(),
        "Nabi binary should exist at {:?}",
        nabi_binary
    );

    let temp_dir = TempDir::new().unwrap();
    let output_file = temp_dir.path().join("test_completion.zsh");

    let output = Command::new(&nabi_binary)
        .args(&[
            "completions",
            "zsh",
            "--output",
            &output_file.to_string_lossy(),
        ])
        .output()
        .expect("Failed to run nabi completions");

    assert!(
        output.status.success(),
        "Completion generation should succeed"
    );

    // Verify the file was created
    assert!(output_file.exists(), "Output file should be created");

    // Read and verify content
    let content = fs::read_to_string(&output_file).unwrap();

    // Should contain static completions
    assert!(content.contains("#compdef nabi"));
    assert!(content.contains("_nabi()"));
    assert!(content.contains("autoload -U is-at-least"));

    // Should contain dynamic completion enhancements
    assert!(content.contains("Dynamic tool completion function"));
    assert!(content.contains("_nabi_dynamic_tools"));
    assert!(content.contains("_nabi__tool__exec_commands"));
    assert!(content.contains("_nabi__exec_commands"));

    // Should contain the tool list command in the dynamic function
    assert!(content.contains("nabi tool list --format=json"));
    assert!(content.contains("jq -r '.tools[]"));
}

/// Test the full completion pipeline for Bash
#[test]
fn test_full_completion_pipeline_bash() {
    let nabi_binary = get_nabi_binary();
    assert!(
        nabi_binary.exists(),
        "Nabi binary should exist at {:?}",
        nabi_binary
    );

    let temp_dir = TempDir::new().unwrap();
    let output_file = temp_dir.path().join("test_completion.bash");

    let output = Command::new(&nabi_binary)
        .args(&[
            "completions",
            "bash",
            "--output",
            &output_file.to_string_lossy(),
        ])
        .output()
        .expect("Failed to run nabi completions");

    assert!(output.status.success());

    let content = fs::read_to_string(&output_file).unwrap();

    // Should contain static bash completions (may have multiple complete statements)
    assert!(content.contains("complete -F"));
    assert!(content.contains("_nabi()"));

    // Should contain dynamic completion enhancements
    assert!(content.contains("Dynamic tool completion function"));
    assert!(content.contains("_nabi_dynamic_tools"));
    assert!(content.contains("_nabi_tool_exec"));
    assert!(content.contains("_nabi_exec"));
}

/// Test completion generation to stdout
#[test]
fn test_completion_to_stdout() {
    let nabi_binary = get_nabi_binary();
    assert!(
        nabi_binary.exists(),
        "Nabi binary should exist at {:?}",
        nabi_binary
    );

    let output = Command::new(&nabi_binary)
        .args(&["completions", "zsh"])
        .output()
        .expect("Failed to run nabi completions");

    assert!(
        output.status.success(),
        "Completion generation to stdout should succeed"
    );

    let stdout = String::from_utf8(output.stdout).unwrap();
    assert!(!stdout.is_empty());
    assert!(stdout.contains("#compdef nabi"));
}

/// Test that completion files are reasonable in size
#[test]
fn test_completion_file_sizes() {
    let nabi_binary = get_nabi_binary();
    assert!(
        nabi_binary.exists(),
        "Nabi binary should exist at {:?}",
        nabi_binary
    );

    let temp_dir = TempDir::new().unwrap();

    let shells = vec!["bash", "zsh"];

    for shell in shells {
        let output_file = temp_dir.path().join(format!("completion_{}.txt", shell));

        let output = Command::new(&nabi_binary)
            .args(&[
                "completions",
                shell,
                "--output",
                &output_file.to_string_lossy(),
            ])
            .output()
            .expect("Failed to run nabi completions");

        assert!(output.status.success());

        let metadata = fs::metadata(&output_file).unwrap();
        let size = metadata.len();

        // Completions should be substantial but not enormous
        // Note: Dynamic completion additions make these much larger than basic clap completions
        assert!(size > 10000, "Completion for {} should be > 10KB", shell);
        assert!(size < 500000, "Completion for {} should be < 500KB", shell);
    }
}

/// Test that dynamic completion functions contain expected tool discovery logic
#[test]
fn test_dynamic_completion_content() {
    let nabi_binary = get_nabi_binary();
    assert!(
        nabi_binary.exists(),
        "Nabi binary should exist at {:?}",
        nabi_binary
    );

    let temp_dir = TempDir::new().unwrap();
    let output_file = temp_dir.path().join("zsh_completion");

    let output = Command::new(&nabi_binary)
        .args(&[
            "completions",
            "zsh",
            "--output",
            &output_file.to_string_lossy(),
        ])
        .output()
        .expect("Failed to run nabi completions");

    assert!(output.status.success());

    let content = fs::read_to_string(&output_file).unwrap();

    // Verify the dynamic function calls the right command
    assert!(content.contains("nabi tool list --format=json"));
    assert!(content.contains("jq -r '.tools[] | .id + \":\" + .description'"));

    // Verify zsh-specific syntax
    assert!(content.contains("_describe 'registered tools' tools"));
    assert!(content.contains("tools=(${(f)\"$(nabi tool list"));
}

/// Test bash dynamic completion content
#[test]
fn test_bash_dynamic_completion_content() {
    let nabi_binary = get_nabi_binary();
    assert!(
        nabi_binary.exists(),
        "Nabi binary should exist at {:?}",
        nabi_binary
    );

    let temp_dir = TempDir::new().unwrap();
    let output_file = temp_dir.path().join("bash_completion");

    let output = Command::new(&nabi_binary)
        .args(&[
            "completions",
            "bash",
            "--output",
            &output_file.to_string_lossy(),
        ])
        .output()
        .expect("Failed to run nabi completions");

    assert!(output.status.success());

    let content = fs::read_to_string(&output_file).unwrap();

    // Verify bash-specific dynamic completion
    assert!(content.contains("nabi tool list --format=json"));
    assert!(content.contains("jq -r '.tools[].id'"));
    assert!(content.contains("COMPREPLY=($(compgen -W"));
    assert!(content.contains("_nabi_dynamic_tools"));
}

/// Test that generated completions include all expected subcommand structures
#[test]
fn test_completion_subcommand_coverage() {
    let nabi_binary = get_nabi_binary();
    assert!(
        nabi_binary.exists(),
        "Nabi binary should exist at {:?}",
        nabi_binary
    );

    let temp_dir = TempDir::new().unwrap();
    let output_file = temp_dir.path().join("completion");

    let output = Command::new(&nabi_binary)
        .args(&[
            "completions",
            "zsh",
            "--output",
            &output_file.to_string_lossy(),
        ])
        .output()
        .expect("Failed to run nabi completions");

    assert!(output.status.success());

    let content = fs::read_to_string(&output_file).unwrap();

    // Test that complex nested subcommands are represented
    // These are generated by clap_complete based on the CLI structure
    let expected_patterns = vec![
        "'claude:Claude domain operations",
        "'data:Data operations",
        "'federation:Federation coordination",
        "'self:Self-management",
        "'tool:Tool registry operations'",
        "'health:Health check operations",
    ];

    for pattern in expected_patterns {
        assert!(
            content.contains(pattern),
            "Completion should include subcommand: {}",
            pattern
        );
    }
}

/// Test that completion commands fail gracefully with invalid shells
#[test]
fn test_invalid_shell_handling() {
    let nabi_binary = get_nabi_binary();
    assert!(
        nabi_binary.exists(),
        "Nabi binary should exist at {:?}",
        nabi_binary
    );

    let output = Command::new(&nabi_binary)
        .args(&["completions", "invalid_shell"])
        .output()
        .expect("Failed to run nabi completions");

    // Should fail with a helpful error message
    assert!(!output.status.success());
    let stderr = String::from_utf8(output.stderr).unwrap();
    assert!(stderr.contains("error") || stderr.contains("invalid"));
}
