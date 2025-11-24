/// Integration tests for MCP federation CLI commands
/// Tests write-local pattern, DLQ fallback, and command execution

use std::fs;
use std::path::PathBuf;
use std::process::Command;
use tempfile::TempDir;

/// Get path to nabi binary
fn nabi_bin() -> String {
    // Use CARGO_BIN_EXE_nabi environment variable set by cargo test
    std::env::var("CARGO_BIN_EXE_nabi")
        .unwrap_or_else(|_| {
            // Fallback to building the path manually
            let mut path = PathBuf::from(env!("CARGO_MANIFEST_DIR"));
            path.push("target");
            path.push("debug");
            path.push("nabi");
            path.to_string_lossy().to_string()
        })
}

/// Setup test environment with temporary directories
fn setup_test_env() -> (TempDir, TempDir, TempDir) {
    let data_dir = TempDir::new().unwrap();
    let state_dir = TempDir::new().unwrap();
    let config_dir = TempDir::new().unwrap();
    (data_dir, state_dir, config_dir)
}

#[test]
fn test_publish_event_basic() {
    let (data_dir, state_dir, _config_dir) = setup_test_env();

    // Create event store directory
    let events_dir = data_dir.path().join("nabi").join("events");
    fs::create_dir_all(&events_dir).unwrap();

    let output = Command::new(nabi_bin())
        .env("XDG_DATA_HOME", data_dir.path())
        .env("XDG_STATE_HOME", state_dir.path())
        .arg("mcp")
        .arg("publish-event")
        .arg("--source")
        .arg("agent:test")
        .arg("--severity")
        .arg("info")
        .arg("--message")
        .arg("Test event message")
        .output()
        .expect("Failed to execute command");

    // Should succeed
    assert!(
        output.status.success(),
        "Command failed: {}",
        String::from_utf8_lossy(&output.stderr)
    );

    // Check that event was written to event_stream.jsonl
    let event_stream = events_dir.join("event_stream.jsonl");
    assert!(event_stream.exists(), "Event stream file should exist");

    let content = fs::read_to_string(&event_stream).unwrap();
    assert!(content.contains("Test event message"));
    assert!(content.contains("agent:test"));
    assert!(content.contains("evt_")); // Event ID prefix
}

#[test]
fn test_publish_event_with_metadata() {
    let (data_dir, state_dir, _config_dir) = setup_test_env();

    let events_dir = data_dir.path().join("nabi").join("events");
    fs::create_dir_all(&events_dir).unwrap();

    let metadata = r#"{"key": "value", "number": 42}"#;

    let output = Command::new(nabi_bin())
        .env("XDG_DATA_HOME", data_dir.path())
        .env("XDG_STATE_HOME", state_dir.path())
        .arg("mcp")
        .arg("publish-event")
        .arg("--source")
        .arg("system:kernel")
        .arg("--severity")
        .arg("warning")
        .arg("--message")
        .arg("Test with metadata")
        .arg("--metadata")
        .arg(metadata)
        .output()
        .expect("Failed to execute command");

    assert!(
        output.status.success(),
        "Command failed: {}",
        String::from_utf8_lossy(&output.stderr)
    );

    let event_stream = events_dir.join("event_stream.jsonl");
    let content = fs::read_to_string(&event_stream).unwrap();
    assert!(content.contains("mcp_exposed"));
    assert!(content.contains("\"key\":\"value\""));
}

#[test]
fn test_publish_event_invalid_source() {
    let (data_dir, state_dir, _config_dir) = setup_test_env();

    // Source without colon should fail
    let output = Command::new(nabi_bin())
        .env("XDG_DATA_HOME", data_dir.path())
        .env("XDG_STATE_HOME", state_dir.path())
        .arg("mcp")
        .arg("publish-event")
        .arg("--source")
        .arg("invalid")
        .arg("--severity")
        .arg("info")
        .arg("--message")
        .arg("Test")
        .output()
        .expect("Failed to execute command");

    assert!(!output.status.success(), "Should fail with invalid source");
    let stderr = String::from_utf8_lossy(&output.stderr);
    assert!(stderr.contains("type:id"));
}

#[test]
fn test_publish_event_invalid_severity() {
    let (data_dir, state_dir, _config_dir) = setup_test_env();

    let output = Command::new(nabi_bin())
        .env("XDG_DATA_HOME", data_dir.path())
        .env("XDG_STATE_HOME", state_dir.path())
        .arg("mcp")
        .arg("publish-event")
        .arg("--source")
        .arg("agent:test")
        .arg("--severity")
        .arg("invalid")
        .arg("--message")
        .arg("Test")
        .output()
        .expect("Failed to execute command");

    assert!(
        !output.status.success(),
        "Should fail with invalid severity"
    );
}

#[test]
fn test_dispatch_task_basic() {
    let (_data_dir, state_dir, _config_dir) = setup_test_env();

    let signals_dir = state_dir.path().join("nabi").join("signals");
    fs::create_dir_all(&signals_dir).unwrap();

    let parameters = r#"{"phase": "2B", "target_latency_ms": 20}"#;

    let output = Command::new(nabi_bin())
        .env("XDG_STATE_HOME", state_dir.path())
        .arg("mcp")
        .arg("dispatch-task")
        .arg("--agent")
        .arg("rust-smith")
        .arg("--task-type")
        .arg("implementation")
        .arg("--parameters")
        .arg(parameters)
        .arg("--priority")
        .arg("8")
        .output()
        .expect("Failed to execute command");

    assert!(
        output.status.success(),
        "Command failed: {}",
        String::from_utf8_lossy(&output.stderr)
    );

    // Check that signal file was created
    let signal_files: Vec<_> = fs::read_dir(&signals_dir)
        .unwrap()
        .filter_map(|e| e.ok())
        .collect();

    assert_eq!(signal_files.len(), 1, "Should create one signal file");

    let signal_content = fs::read_to_string(signal_files[0].path()).unwrap();
    assert!(signal_content.contains("rust-smith"));
    assert!(signal_content.contains("implementation"));
    assert!(signal_content.contains("task_"));
    assert!(signal_content.contains("\"priority\":8"));
}

#[test]
fn test_dispatch_task_invalid_agent() {
    let (_data_dir, state_dir, _config_dir) = setup_test_env();

    // Agent with uppercase should fail
    let output = Command::new(nabi_bin())
        .env("XDG_STATE_HOME", state_dir.path())
        .arg("mcp")
        .arg("dispatch-task")
        .arg("--agent")
        .arg("Invalid-Agent")
        .arg("--task-type")
        .arg("test")
        .arg("--parameters")
        .arg(r#"{"key": "value"}"#)
        .output()
        .expect("Failed to execute command");

    assert!(!output.status.success(), "Should fail with invalid agent");
}

#[test]
fn test_dispatch_task_invalid_parameters() {
    let (_data_dir, state_dir, _config_dir) = setup_test_env();

    // Non-object parameters should fail
    let output = Command::new(nabi_bin())
        .env("XDG_STATE_HOME", state_dir.path())
        .arg("mcp")
        .arg("dispatch-task")
        .arg("--agent")
        .arg("test-agent")
        .arg("--task-type")
        .arg("test")
        .arg("--parameters")
        .arg(r#"["array", "not", "object"]"#)
        .output()
        .expect("Failed to execute command");

    assert!(
        !output.status.success(),
        "Should fail with non-object parameters"
    );
}

#[test]
fn test_ack_event_basic() {
    let (data_dir, state_dir, _config_dir) = setup_test_env();

    // First publish an event
    let events_dir = data_dir.path().join("nabi").join("events");
    fs::create_dir_all(&events_dir).unwrap();

    let publish_output = Command::new(nabi_bin())
        .env("XDG_DATA_HOME", data_dir.path())
        .env("XDG_STATE_HOME", state_dir.path())
        .arg("mcp")
        .arg("publish-event")
        .arg("--source")
        .arg("agent:test")
        .arg("--severity")
        .arg("info")
        .arg("--message")
        .arg("Event to ack")
        .output()
        .expect("Failed to execute command");

    assert!(publish_output.status.success());

    // Extract event ID from output (it's in the JSON stderr)
    let stderr = String::from_utf8_lossy(&publish_output.stderr);
    let event_id = stderr
        .lines()
        .find(|l| l.contains("event_id"))
        .and_then(|l| {
            l.split("\"event_id\":")
                .nth(1)
                .and_then(|s| s.split('"').nth(1))
        })
        .expect("Should find event_id in output");

    // Now acknowledge it
    let ack_output = Command::new(nabi_bin())
        .env("XDG_DATA_HOME", data_dir.path())
        .env("XDG_STATE_HOME", state_dir.path())
        .arg("mcp")
        .arg("ack-event")
        .arg("--event-id")
        .arg(event_id)
        .arg("--status")
        .arg("acknowledged")
        .output()
        .expect("Failed to execute command");

    assert!(
        ack_output.status.success(),
        "Ack command failed: {}",
        String::from_utf8_lossy(&ack_output.stderr)
    );

    // Check ack file was created
    let acks_dir = state_dir.path().join("nabi").join("acks");
    assert!(acks_dir.exists());

    let ack_file = acks_dir.join(format!("{}.json", event_id));
    assert!(ack_file.exists(), "Ack file should exist");

    let ack_content = fs::read_to_string(&ack_file).unwrap();
    assert!(ack_content.contains(event_id));
    assert!(ack_content.contains("acknowledged"));
    assert!(ack_content.contains("ack-"));
}

#[test]
fn test_stream_events_empty() {
    let (data_dir, _state_dir, _config_dir) = setup_test_env();

    let output = Command::new(nabi_bin())
        .env("XDG_DATA_HOME", data_dir.path())
        .arg("mcp")
        .arg("stream-events")
        .output()
        .expect("Failed to execute command");

    assert!(output.status.success());

    let stdout = String::from_utf8_lossy(&output.stdout);
    assert!(stdout.contains("\"events\":[]"));
    assert!(stdout.contains("\"total\":0"));
}

#[test]
fn test_stream_events_with_filtering() {
    let (data_dir, state_dir, _config_dir) = setup_test_env();

    let events_dir = data_dir.path().join("nabi").join("events");
    fs::create_dir_all(&events_dir).unwrap();

    // Publish multiple events
    for i in 0..3 {
        let source = if i == 0 {
            "agent:igris"
        } else {
            "agent:beru"
        };
        let severity = if i < 2 { "info" } else { "warning" };

        Command::new(nabi_bin())
            .env("XDG_DATA_HOME", data_dir.path())
            .env("XDG_STATE_HOME", state_dir.path())
            .arg("mcp")
            .arg("publish-event")
            .arg("--source")
            .arg(source)
            .arg("--severity")
            .arg(severity)
            .arg("--message")
            .arg(&format!("Event {}", i))
            .output()
            .expect("Failed to execute command");
    }

    // Stream with source filter
    let output = Command::new(nabi_bin())
        .env("XDG_DATA_HOME", data_dir.path())
        .arg("mcp")
        .arg("stream-events")
        .arg("--source")
        .arg("agent:igris")
        .output()
        .expect("Failed to execute command");

    assert!(output.status.success());

    let stdout = String::from_utf8_lossy(&output.stdout);
    let json: serde_json::Value = serde_json::from_str(&stdout).unwrap();

    assert_eq!(json["filtered"].as_u64().unwrap(), 1);
    assert_eq!(json["events"].as_array().unwrap().len(), 1);
}

#[test]
fn test_write_local_performance() {
    // This is a basic smoke test - real performance tests would use criterion
    let (data_dir, state_dir, _config_dir) = setup_test_env();

    let events_dir = data_dir.path().join("nabi").join("events");
    fs::create_dir_all(&events_dir).unwrap();

    let start = std::time::Instant::now();

    let output = Command::new(nabi_bin())
        .env("XDG_DATA_HOME", data_dir.path())
        .env("XDG_STATE_HOME", state_dir.path())
        .arg("mcp")
        .arg("publish-event")
        .arg("--source")
        .arg("agent:perf-test")
        .arg("--severity")
        .arg("info")
        .arg("--message")
        .arg("Performance test")
        .output()
        .expect("Failed to execute command");

    let elapsed = start.elapsed();

    assert!(output.status.success());

    // Very loose check - real performance testing needs proper benchmarking
    // This just ensures we're not egregiously slow
    assert!(
        elapsed.as_millis() < 1000,
        "Command took too long: {:?}",
        elapsed
    );
}
