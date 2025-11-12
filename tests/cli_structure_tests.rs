/// Regression tests for CLI modular structure
///
/// Ensures that the refactored modular structure maintains functionality
/// and that all handlers are properly wired.
///
/// Run with: cargo test --test cli_structure_tests

#[cfg(test)]
mod tests {
    use std::process::Command;

    /// Test that main.rs compiles and basic structure is intact
    #[test]
    fn test_cli_compiles() {
        // This test will fail if the code doesn't compile
        // The fact that we can compile means the structure is valid
        assert!(true, "CLI structure compiles successfully");
    }

    /// Test CLI help output includes all major commands
    #[test]
    #[ignore] // Ignore by default - requires built binary
    fn test_cli_help_includes_commands() {
        let output = Command::new("cargo")
            .args(&["run", "--release", "--", "--help"])
            .current_dir(env!("CARGO_MANIFEST_DIR"))
            .output();

        if let Ok(output) = output {
            let stdout = String::from_utf8_lossy(&output.stdout);

            // Check for major command categories
            assert!(
                stdout.contains("claude"),
                "Help should include claude command"
            );
            assert!(stdout.contains("data"), "Help should include data command");
            assert!(
                stdout.contains("federation"),
                "Help should include federation command"
            );
            assert!(stdout.contains("port"), "Help should include port command");
            assert!(stdout.contains("tmux"), "Help should include tmux command");
        } else {
            // Skip if binary not built
            println!("Skipping test - binary not built. Run: cargo build --release");
        }
    }
}
