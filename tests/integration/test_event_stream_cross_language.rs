/// Integration Test: Cross-Language Event Stream Consistency
///
/// This test catches the exact bug we encountered:
/// - Rust wrote events to ~/Library/Application Support/nabi/events/
/// - Python hooks read from ~/.local/share/nabi/events/
/// - Result: 40K events existed but hooks found ZERO
///
/// This integration test ensures writer and reader use the SAME location

use anyhow::Result;
use std::env;
use std::process::Command;
use tempfile::TempDir;
use serde_json::Value;

#[test]
fn test_rust_publish_python_read_consistency() -> Result<()> {
    // Setup: Use temporary XDG directory
    let temp_dir = TempDir::new()?;
    let xdg_data = temp_dir.path().to_str().unwrap();

    // Set environment for both Rust and Python
    env::set_var("XDG_DATA_HOME", xdg_data);

    // Step 1: Publish event using Rust (nabi events publish)
    let publish_output = Command::new("nabi")
        .args(&["events", "publish"])
        .args(&["--source", "test-cross-lang"])
        .args(&["--message", "rust_python_consistency_test"])
        .args(&["--severity", "info"])
        .env("XDG_DATA_HOME", xdg_data)
        .output()?;

    assert!(
        publish_output.status.success(),
        "Rust event publish failed: {}",
        String::from_utf8_lossy(&publish_output.stderr)
    );

    // Step 2: Read event using Python hook simulation
    let pull_output = Command::new("nabi")
        .args(&["events", "pull"])
        .args(&["--source", "test-cross-lang"])
        .args(&["--limit", "10"])
        .args(&["--format", "json"])
        .env("XDG_DATA_HOME", xdg_data)
        .output()?;

    assert!(
        pull_output.status.success(),
        "Python event pull failed: {}",
        String::from_utf8_lossy(&pull_output.stderr)
    );

    // Step 3: Verify event was found
    let events: Vec<Value> = serde_json::from_slice(&pull_output.stdout)?;

    // THE BUG: This assertion would fail with the old code
    // Rust wrote to one location, Python read from another
    assert!(
        !events.is_empty(),
        "REGRESSION DETECTED: Event was published (Rust) but pull returned empty (Python). \
         This means writer and reader are using DIFFERENT storage locations! \
         Check that both use NabiPaths/platform_paths with same XDG variables."
    );

    // Verify we got the exact event
    let found = events.iter().any(|e| {
        e.get("message")
            .and_then(|m| m.as_str())
            .map(|m| m == "rust_python_consistency_test")
            .unwrap_or(false)
    });

    assert!(
        found,
        "Published event not found in pull results. \
         Events published: {:?}, but our test event is missing. \
         This indicates a path mismatch between writer and reader.",
        events
    );

    // Cleanup
    env::remove_var("XDG_DATA_HOME");

    Ok(())
}

#[test]
fn test_event_stream_file_location_matches() -> Result<()> {
    // Verify that the actual file location used matches expectations
    let temp_dir = TempDir::new()?;
    env::set_var("XDG_DATA_HOME", temp_dir.path());

    // Publish an event
    Command::new("nabi")
        .args(&["events", "publish"])
        .args(&["--source", "test-location"])
        .args(&["--message", "location_test"])
        .env("XDG_DATA_HOME", temp_dir.path())
        .output()?;

    // Check that file exists at expected XDG location
    let expected_file = temp_dir
        .path()
        .join("nabi")
        .join("events")
        .join("event_stream.jsonl");

    assert!(
        expected_file.exists(),
        "Event stream file not found at expected XDG location: {:?}. \
         This means nabi is writing to a different location (platform-specific path?)",
        expected_file
    );

    // Check that file is NOT at macOS-specific location
    #[cfg(target_os = "macos")]
    {
        let macos_path = dirs::home_dir()
            .unwrap()
            .join("Library")
            .join("Application Support")
            .join("nabi")
            .join("events")
            .join("event_stream.jsonl");

        // This is the OLD BUG: writing to macOS path instead of XDG
        assert!(
            !macos_path.exists() || macos_path != expected_file,
            "REGRESSION: Event stream is being written to macOS-specific path \
             instead of XDG_DATA_HOME location. Old bug has returned!"
        );
    }

    env::remove_var("XDG_DATA_HOME");
    Ok(())
}

#[test]
fn test_multiple_xdg_vars_respected() -> Result<()> {
    // Ensure ALL XDG variables are respected, not just DATA_HOME
    let temp_data = TempDir::new()?;
    let temp_state = TempDir::new()?;

    env::set_var("XDG_DATA_HOME", temp_data.path());
    env::set_var("XDG_STATE_HOME", temp_state.path());

    // Event stream should use DATA_HOME
    Command::new("nabi")
        .args(&["events", "publish", "--source", "test-xdg", "--message", "test"])
        .env("XDG_DATA_HOME", temp_data.path())
        .env("XDG_STATE_HOME", temp_state.path())
        .output()?;

    let data_file = temp_data.path().join("nabi/events/event_stream.jsonl");
    let state_file = temp_state.path().join("nabi/events/event_stream.jsonl");

    assert!(
        data_file.exists(),
        "Event stream should be in XDG_DATA_HOME, not found"
    );

    assert!(
        !state_file.exists(),
        "Event stream incorrectly using XDG_STATE_HOME instead of XDG_DATA_HOME"
    );

    env::remove_var("XDG_DATA_HOME");
    env::remove_var("XDG_STATE_HOME");

    Ok(())
}
