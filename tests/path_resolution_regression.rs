/// Regression Tests for XDG Path Resolution
///
/// Bug: Event stream writer (Rust) and reader (Python hooks) used different paths
/// Root Cause: Rust used dirs::data_local_dir() (platform-specific)
///              Python expected XDG-compliant ~/.local/share/
/// Result: 40K events written to ~/Library/Application Support/ on macOS
///         but hooks looked in ~/.local/share/ → found ZERO events
///
/// These tests prevent regression by ensuring:
/// 1. Rust uses NabiPaths abstraction (not raw dirs)
/// 2. XDG environment variables are respected
/// 3. Writer and reader paths match
/// 4. Platform-specific behavior is documented and tested

use anyhow::Result;
use std::env;
use std::fs;
use std::path::PathBuf;
use tempfile::TempDir;

// Import the paths module we're testing
use nabi::paths::NabiPaths;

#[test]
fn test_data_dir_respects_xdg_data_home() {
    // Setup: Set XDG_DATA_HOME to test directory
    let temp_dir = TempDir::new().unwrap();
    let test_path = temp_dir.path().to_str().unwrap();

    env::set_var("XDG_DATA_HOME", test_path);

    // Execute: Get data directory
    let result = NabiPaths::data_dir().unwrap();

    // Verify: Should use XDG_DATA_HOME, not platform default
    assert_eq!(
        result,
        PathBuf::from(test_path).join("nabi"),
        "NabiPaths::data_dir() must respect XDG_DATA_HOME environment variable"
    );

    // Cleanup
    env::remove_var("XDG_DATA_HOME");
}

#[test]
fn test_data_dir_uses_xdg_default_when_env_not_set() {
    // Setup: Remove XDG_DATA_HOME
    env::remove_var("XDG_DATA_HOME");

    // Execute: Get data directory
    let result = NabiPaths::data_dir().unwrap();

    // Verify: Should use ~/.local/share/nabi (XDG default, NOT macOS-specific path)
    let expected = dirs::home_dir()
        .unwrap()
        .join(".local")
        .join("share")
        .join("nabi");

    assert_eq!(
        result, expected,
        "Without XDG_DATA_HOME, must use XDG default ~/.local/share/nabi, \
         NOT platform-specific paths like ~/Library/Application Support"
    );
}

#[test]
fn test_state_dir_respects_xdg_state_home() {
    let temp_dir = TempDir::new().unwrap();
    let test_path = temp_dir.path().to_str().unwrap();

    env::set_var("XDG_STATE_HOME", test_path);

    let result = NabiPaths::state_dir().unwrap();

    assert_eq!(
        result,
        PathBuf::from(test_path).join("nabi"),
        "NabiPaths::state_dir() must respect XDG_STATE_HOME"
    );

    env::remove_var("XDG_STATE_HOME");
}

#[test]
fn test_cache_dir_respects_xdg_cache_home() {
    let temp_dir = TempDir::new().unwrap();
    let test_path = temp_dir.path().to_str().unwrap();

    env::set_var("XDG_CACHE_HOME", test_path);

    let result = NabiPaths::cache_dir().unwrap();

    assert_eq!(
        result,
        PathBuf::from(test_path).join("nabi"),
        "NabiPaths::cache_dir() must respect XDG_CACHE_HOME"
    );

    env::remove_var("XDG_CACHE_HOME");
}

#[test]
fn test_config_dir_respects_xdg_config_home() {
    let temp_dir = TempDir::new().unwrap();
    let test_path = temp_dir.path().to_str().unwrap();

    env::set_var("XDG_CONFIG_HOME", test_path);

    let result = NabiPaths::config_dir().unwrap();

    assert_eq!(
        result,
        PathBuf::from(test_path).join("nabi"),
        "NabiPaths::config_dir() must respect XDG_CONFIG_HOME"
    );

    env::remove_var("XDG_CONFIG_HOME");
}

#[test]
fn test_event_store_uses_nabi_paths() {
    // This test ensures events module uses NabiPaths, not raw dirs
    use nabi::commands::events::get_event_store_path_for_test;

    let temp_dir = TempDir::new().unwrap();
    env::set_var("XDG_DATA_HOME", temp_dir.path());

    let event_path = get_event_store_path_for_test().unwrap();

    // Should be: $XDG_DATA_HOME/nabi/events/event_stream.jsonl
    assert!(
        event_path.starts_with(temp_dir.path()),
        "Event store path must use NabiPaths::data_dir(), not dirs::data_local_dir(). \
         Got: {:?}, Expected to start with: {:?}",
        event_path,
        temp_dir.path()
    );

    env::remove_var("XDG_DATA_HOME");
}

#[test]
fn test_all_storage_types_use_xdg_pattern() {
    // Regression: Ensure ALL storage uses XDG pattern
    let temp_data = TempDir::new().unwrap();
    let temp_state = TempDir::new().unwrap();
    let temp_cache = TempDir::new().unwrap();
    let temp_config = TempDir::new().unwrap();

    env::set_var("XDG_DATA_HOME", temp_data.path());
    env::set_var("XDG_STATE_HOME", temp_state.path());
    env::set_var("XDG_CACHE_HOME", temp_cache.path());
    env::set_var("XDG_CONFIG_HOME", temp_config.path());

    let data_dir = NabiPaths::data_dir().unwrap();
    let state_dir = NabiPaths::state_dir().unwrap();
    let cache_dir = NabiPaths::cache_dir().unwrap();
    let config_dir = NabiPaths::config_dir().unwrap();

    // All should respect their XDG environment variables
    assert!(data_dir.starts_with(temp_data.path()));
    assert!(state_dir.starts_with(temp_state.path()));
    assert!(cache_dir.starts_with(temp_cache.path()));
    assert!(config_dir.starts_with(temp_config.path()));

    // All should end with "nabi"
    assert_eq!(data_dir.file_name().unwrap(), "nabi");
    assert_eq!(state_dir.file_name().unwrap(), "nabi");
    assert_eq!(cache_dir.file_name().unwrap(), "nabi");
    assert_eq!(config_dir.file_name().unwrap(), "nabi");

    // Cleanup
    env::remove_var("XDG_DATA_HOME");
    env::remove_var("XDG_STATE_HOME");
    env::remove_var("XDG_CACHE_HOME");
    env::remove_var("XDG_CONFIG_HOME");
}

#[test]
#[cfg(target_os = "macos")]
fn test_macos_does_not_use_library_application_support() {
    // CRITICAL: This is the exact bug we had
    // Even on macOS, we should NOT use ~/Library/Application Support
    // when XDG_DATA_HOME is set

    let temp_dir = TempDir::new().unwrap();
    env::set_var("XDG_DATA_HOME", temp_dir.path());

    let data_dir = NabiPaths::data_dir().unwrap();

    // Must NOT contain "Library/Application Support"
    let path_str = data_dir.to_string_lossy();
    assert!(
        !path_str.contains("Library/Application Support"),
        "On macOS, NabiPaths must respect XDG_DATA_HOME, not use Library/Application Support. \
         Got: {}",
        path_str
    );

    env::remove_var("XDG_DATA_HOME");
}

#[test]
fn test_path_consistency_across_modules() {
    // Ensure all modules that need data directory use same path
    use nabi::commands::mcp::common::get_event_store_path_for_test as mcp_path;
    use nabi::commands::events::get_event_store_path_for_test as events_path;

    let temp_dir = TempDir::new().unwrap();
    env::set_var("XDG_DATA_HOME", temp_dir.path());

    let mcp_base = mcp_path().unwrap().parent().unwrap().to_path_buf();
    let events_base = events_path().unwrap().parent().unwrap().to_path_buf();

    assert_eq!(
        mcp_base, events_base,
        "MCP and Events modules must use same base path. \
         This regression test catches modules using different path resolution methods."
    );

    env::remove_var("XDG_DATA_HOME");
}

#[test]
fn test_no_hardcoded_platform_paths_in_modules() {
    // Compile-time check: grep for raw dirs:: usage would be better
    // This is a runtime check that paths don't contain platform-specific strings

    env::remove_var("XDG_DATA_HOME");

    let data_dir = NabiPaths::data_dir().unwrap();
    let path_str = data_dir.to_string_lossy().to_lowercase();

    // Should be platform-agnostic XDG path
    assert!(
        path_str.contains(".local/share") || path_str.contains("xdg"),
        "Data directory should follow XDG pattern. Got: {}",
        path_str
    );
}

#[cfg(test)]
mod integration_tests {
    use super::*;
    use std::process::Command;

    #[test]
    #[ignore] // Run with: cargo test --test path_resolution_regression -- --ignored
    fn test_event_publish_and_pull_use_same_location() {
        // This is the EXACT bug: publish wrote to one location, pull read from another

        let temp_dir = TempDir::new().unwrap();
        env::set_var("XDG_DATA_HOME", temp_dir.path());

        // Publish an event
        let publish_output = Command::new("nabi")
            .args(&["events", "publish"])
            .args(&["--source", "test-regression"])
            .args(&["--message", "path_consistency_test"])
            .output()
            .expect("Failed to publish event");

        assert!(publish_output.status.success(), "Event publish failed");

        // Pull events
        let pull_output = Command::new("nabi")
            .args(&["events", "pull", "--limit", "1", "--format", "json"])
            .output()
            .expect("Failed to pull events");

        assert!(pull_output.status.success(), "Event pull failed");

        let events: Vec<serde_json::Value> =
            serde_json::from_slice(&pull_output.stdout).unwrap();

        // The bug: pull would return empty [] even though publish succeeded
        assert!(
            !events.is_empty(),
            "REGRESSION: Event was published but pull returned empty. \
             This means writer and reader are using different paths!"
        );

        // Verify we got our test event
        let found = events.iter().any(|e| {
            e["message"].as_str() == Some("path_consistency_test")
        });

        assert!(
            found,
            "Published event not found in pull results. \
             Writer and reader using different storage locations!"
        );

        env::remove_var("XDG_DATA_HOME");
    }
}

/// Helper to verify no module uses raw dirs:: calls
/// This would be better as a compile-time check or lint rule
#[test]
fn test_documentation_of_xdg_requirement() {
    // This test documents the requirement
    // Actual enforcement should be via clippy or pre-commit hook

    println!("\n=== XDG Path Resolution Requirement ===");
    println!("ALL modules must use NabiPaths abstraction:");
    println!("  ✓ USE:  NabiPaths::data_dir()");
    println!("  ✓ USE:  NabiPaths::state_dir()");
    println!("  ✓ USE:  NabiPaths::cache_dir()");
    println!("  ✓ USE:  NabiPaths::config_dir()");
    println!("");
    println!("  ✗ NEVER: dirs::data_local_dir()");
    println!("  ✗ NEVER: dirs::data_dir()");
    println!("  ✗ NEVER: dirs::state_dir()");
    println!("  ✗ NEVER: dirs::cache_dir()");
    println!("  ✗ NEVER: dirs::config_dir()");
    println!("\nReason: Platform-specific paths cause writer/reader mismatch");
    println!("See: docs/testing/PATH_RESOLUTION_TEST_SPEC.md");
}
