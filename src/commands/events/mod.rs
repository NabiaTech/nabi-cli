/// Federation Event Bus CLI - Module Organization
pub mod ack;

// Re-export everything from the main events module
use anyhow::{Context, Result};
use chrono::{DateTime, Duration, Local, Utc};
use clap::{Subcommand, ValueEnum};
use serde::{Deserialize, Serialize};
use std::fs::{self, File, OpenOptions};
use std::io::{BufRead, BufReader, Write as IoWrite};
use std::path::PathBuf;
use uuid::Uuid;

use crate::paths::NabiPaths;

#[derive(Subcommand)]
pub enum EventsCommands {
    /// Publish a new event to the federation event bus
    Publish {
        /// Event source (vigil, doc-drift, port-registry, hooks, kernel, etc.)
        #[arg(long, short = 's')]
        source: String,

        /// Event severity level
        #[arg(long, value_enum, default_value = "info")]
        severity: EventSeverity,

        /// Human-readable event message
        #[arg(long, short = 'm')]
        message: String,

        /// Optional metadata as JSON string
        #[arg(long)]
        metadata: Option<String>,

        /// Optional timestamp (defaults to now)
        #[arg(long)]
        timestamp: Option<String>,

        /// Read event from stdin as JSON
        #[arg(long, conflicts_with_all = &["source", "severity", "message"])]
        stdin: bool,
    },

    /// List events (TUI-friendly JSON output)
    List {
        /// Filter by event source(s), comma-separated
        #[arg(long)]
        source: Option<String>,

        /// Filter by severity level(s), comma-separated
        #[arg(long)]
        severity: Option<String>,

        /// Filter by acknowledged status
        #[arg(long)]
        acknowledged: Option<bool>,

        /// Maximum number of events to return
        #[arg(long, default_value_t = 50)]
        limit: usize,

        /// Always output as JSON (for TUI compatibility)
        #[arg(long)]
        json: bool,
    },

    /// Pull recent events from the event bus
    Pull {
        /// How far back to look (e.g., "1h", "24h", "7d")
        #[arg(long, default_value = "24h")]
        since: String,

        /// Maximum number of events to return
        #[arg(long, default_value_t = 50)]
        limit: usize,

        /// Filter by event source(s), comma-separated
        #[arg(long)]
        source: Option<String>,

        /// Filter by severity level(s), comma-separated
        #[arg(long)]
        severity: Option<String>,

        /// Output format
        #[arg(long, value_enum, default_value = "text")]
        format: OutputFormat,

        /// Filter by correlation ID
        #[arg(long)]
        correlation_id: Option<String>,
    },

    /// Subscribe to live event stream (like tail -f)
    Subscribe {
        /// Follow mode (continuously stream new events)
        #[arg(long, short = 'f')]
        follow: bool,

        /// Filter by event source(s), comma-separated
        #[arg(long)]
        source: Option<String>,

        /// Filter by severity level(s), comma-separated
        #[arg(long)]
        severity: Option<String>,
    },

    /// Clean up old events based on retention policy
    Cleanup {
        /// Dry run (show what would be deleted)
        #[arg(long)]
        dry_run: bool,

        /// Force cleanup even if recent
        #[arg(long)]
        force: bool,
    },

    /// Show event bus statistics
    Stats {
        /// Output format
        #[arg(long, value_enum, default_value = "text")]
        format: OutputFormat,
    },

    /// Acknowledge a federation event
    Ack {
        /// Event ID to acknowledge
        event_id: String,

        /// Optional metadata as JSON
        #[arg(long)]
        metadata: Option<String>,

        /// Output as JSON
        #[arg(long)]
        json: bool,
    },
}

#[derive(Clone, Copy, Debug, PartialEq, Eq, ValueEnum, Serialize, Deserialize)]
#[serde(rename_all = "lowercase")]
pub enum EventSeverity {
    Debug,
    Info,
    Warning,
    Critical,
}

impl EventSeverity {
    fn retention_days(&self) -> i64 {
        match self {
            EventSeverity::Debug => 1,
            EventSeverity::Info => 7,
            EventSeverity::Warning => 30,
            EventSeverity::Critical => 90,
        }
    }

    fn color_code(&self) -> &'static str {
        match self {
            EventSeverity::Debug => "\x1b[90m",    // Gray
            EventSeverity::Info => "\x1b[34m",     // Blue
            EventSeverity::Warning => "\x1b[33m",  // Yellow
            EventSeverity::Critical => "\x1b[31m", // Red
        }
    }
}

#[derive(Clone, Copy, Debug, PartialEq, Eq, ValueEnum)]
pub enum OutputFormat {
    Text,
    Json,
    Compact,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Event {
    /// Unique event identifier (UUID)
    pub id: String,
    pub source: String,
    pub severity: EventSeverity,
    pub message: String,
    pub timestamp: DateTime<Utc>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub metadata: Option<serde_json::Value>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub correlation_id: Option<String>,
}

pub fn handle_events_commands(cmd: EventsCommands) -> Result<()> {
    match cmd {
        EventsCommands::Publish {
            source,
            severity,
            message,
            metadata,
            timestamp,
            stdin,
        } => handle_publish(source, severity, message, metadata, timestamp, stdin),
        EventsCommands::List {
            source,
            severity,
            acknowledged,
            limit,
            json,
        } => handle_list(source, severity, acknowledged, limit, json),
        EventsCommands::Pull {
            since,
            limit,
            source,
            severity,
            format,
            correlation_id,
        } => handle_pull(since, limit, source, severity, format, correlation_id),
        EventsCommands::Subscribe {
            follow,
            source,
            severity,
        } => handle_subscribe(follow, source, severity),
        EventsCommands::Cleanup { dry_run, force } => handle_cleanup(dry_run, force),
        EventsCommands::Stats { format } => handle_stats(format),
        EventsCommands::Ack {
            event_id,
            metadata,
            json,
        } => {
            let metadata_value = metadata.map(|s| serde_json::from_str(&s)).transpose()?;
            ack::execute_ack(&event_id, metadata_value, json)
        }
    }
}

fn get_event_store_path() -> Result<PathBuf> {
    let state_dir = NabiPaths::data_dir()?.join("events");
    fs::create_dir_all(&state_dir)?;
    Ok(state_dir.join("event_stream.jsonl"))
}

fn handle_publish(
    source: String,
    severity: EventSeverity,
    message: String,
    metadata: Option<String>,
    timestamp: Option<String>,
    stdin: bool,
) -> Result<()> {
    // Generate UUID for this event
    let event_id = Uuid::new_v4().to_string();

    let mut event = if stdin {
        // Read event from stdin
        let stdin = std::io::stdin();
        let reader = stdin.lock();
        let mut event: Event =
            serde_json::from_reader(reader).context("Failed to parse event JSON from stdin")?;
        // Override ID even if provided in stdin (ensure uniqueness)
        event.id = event_id.clone();
        event
    } else {
        // Build event from CLI args
        let parsed_metadata = metadata
            .map(|m| serde_json::from_str(&m))
            .transpose()
            .context("Failed to parse metadata JSON")?;

        let parsed_timestamp = timestamp
            .map(|t| DateTime::parse_from_rfc3339(&t).map(|dt| dt.with_timezone(&Utc)))
            .transpose()
            .context("Failed to parse timestamp")?
            .unwrap_or_else(Utc::now);

        Event {
            id: event_id.clone(),
            source,
            severity,
            message,
            timestamp: parsed_timestamp,
            metadata: parsed_metadata,
            correlation_id: None,
        }
    };

    // DUAL STORAGE PATTERN:
    // 1. Write to JSONL stream (backward compatibility)
    // 2. Write to individual event file (enables acknowledgment)

    // Storage 1: Append to JSONL stream
    let store_path = get_event_store_path()?;
    let mut file = OpenOptions::new()
        .create(true)
        .append(true)
        .open(&store_path)
        .context("Failed to open event store")?;

    let json_line = serde_json::to_string(&event)?;
    writeln!(file, "{}", json_line).context("Failed to write event")?;

    // Storage 2: Write to date-based individual file
    let state_dir = NabiPaths::data_dir()?
        .join("events")
        .join(event.timestamp.format("%Y-%m-%d").to_string());

    fs::create_dir_all(&state_dir)?;

    let event_file_path = state_dir.join(format!("{}.json", event.id));
    let event_json = serde_json::to_string_pretty(&event)?;
    fs::write(&event_file_path, event_json).context("Failed to write individual event file")?;

    println!(
        "{}✓{} Event published: [{}] {} (ID: {})",
        "\x1b[32m", "\x1b[0m", event.source, event.message, event.id
    );

    Ok(())
}

fn handle_list(
    source_filter: Option<String>,
    severity_filter: Option<String>,
    _acknowledged_filter: Option<bool>,
    limit: usize,
    _json: bool, // Always JSON for TUI
) -> Result<()> {
    let store_path = get_event_store_path()?;

    if !store_path.exists() {
        // Return empty array for TUI
        println!("{{\"events\": []}}");
        return Ok(());
    }

    let file = File::open(&store_path).context("Failed to open event store")?;
    let reader = BufReader::new(file);

    let source_filters: Option<Vec<String>> =
        source_filter.map(|s| s.split(',').map(|x| x.trim().to_string()).collect());

    let severity_filters: Option<Vec<String>> =
        severity_filter.map(|s| s.split(',').map(|x| x.trim().to_lowercase()).collect());

    let mut events: Vec<Event> = Vec::new();

    for line in reader.lines() {
        let line = line?;
        let event: Event = match serde_json::from_str(&line) {
            Ok(e) => e,
            Err(_) => continue, // Skip malformed lines
        };

        // Apply filters
        if let Some(ref sources) = source_filters {
            if !sources.contains(&event.source) {
                continue;
            }
        }

        if let Some(ref severities) = severity_filters {
            let severity_str = format!("{:?}", event.severity).to_lowercase();
            if !severities.contains(&severity_str) {
                continue;
            }
        }

        events.push(event);

        if events.len() >= limit {
            break;
        }
    }

    // Reverse to show newest first
    events.reverse();

    // Output in TUI-expected format
    let output = serde_json::json!({
        "events": events
    });

    println!("{}", serde_json::to_string_pretty(&output)?);

    Ok(())
}

fn parse_duration(duration_str: &str) -> Result<Duration> {
    let duration_str = duration_str.trim();

    if duration_str.ends_with('h') {
        let hours: i64 = duration_str[..duration_str.len() - 1]
            .parse()
            .context("Invalid hour duration")?;
        Ok(Duration::hours(hours))
    } else if duration_str.ends_with('d') {
        let days: i64 = duration_str[..duration_str.len() - 1]
            .parse()
            .context("Invalid day duration")?;
        Ok(Duration::days(days))
    } else if duration_str.ends_with('m') {
        let minutes: i64 = duration_str[..duration_str.len() - 1]
            .parse()
            .context("Invalid minute duration")?;
        Ok(Duration::minutes(minutes))
    } else {
        // Try parsing as ISO8601 timestamp
        let dt = DateTime::parse_from_rfc3339(duration_str)?;
        let now = Utc::now();
        Ok(now.signed_duration_since(dt.with_timezone(&Utc)))
    }
}

fn handle_pull(
    since: String,
    limit: usize,
    source_filter: Option<String>,
    severity_filter: Option<String>,
    format: OutputFormat,
    correlation_id: Option<String>,
) -> Result<()> {
    let store_path = get_event_store_path()?;

    if !store_path.exists() {
        println!("No events found");
        return Ok(());
    }

    let duration = parse_duration(&since)?;
    let cutoff = Utc::now() - duration;

    let file = File::open(&store_path).context("Failed to open event store")?;
    let reader = BufReader::new(file);

    let source_filters: Option<Vec<String>> =
        source_filter.map(|s| s.split(',').map(|x| x.trim().to_string()).collect());

    let severity_filters: Option<Vec<String>> =
        severity_filter.map(|s| s.split(',').map(|x| x.trim().to_lowercase()).collect());

    let mut events: Vec<Event> = Vec::new();

    for line in reader.lines() {
        let line = line?;
        let event: Event = match serde_json::from_str(&line) {
            Ok(e) => e,
            Err(_) => continue, // Skip malformed lines
        };

        // Apply filters
        if event.timestamp < cutoff {
            continue;
        }

        if let Some(ref sources) = source_filters {
            if !sources.contains(&event.source) {
                continue;
            }
        }

        if let Some(ref severities) = severity_filters {
            let severity_str = format!("{:?}", event.severity).to_lowercase();
            if !severities.contains(&severity_str) {
                continue;
            }
        }

        if let Some(ref corr_id) = correlation_id {
            if event.correlation_id.as_ref() != Some(corr_id) {
                continue;
            }
        }

        events.push(event);

        if events.len() >= limit {
            break;
        }
    }

    // Reverse to show newest first
    events.reverse();

    match format {
        OutputFormat::Json => {
            println!("{}", serde_json::to_string_pretty(&events)?);
        }
        OutputFormat::Text => {
            print_events_text(&events);
        }
        OutputFormat::Compact => {
            print_events_compact(&events);
        }
    }

    Ok(())
}

fn print_events_text(events: &[Event]) {
    if events.is_empty() {
        println!("No events found");
        return;
    }

    println!("━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━");
    println!("📡 FEDERATION EVENTS");
    println!("━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━");

    for event in events {
        // Convert UTC to local timezone (respects TZ environment variable)
        let local_time = event.timestamp.with_timezone(&Local);
        let time_str = local_time.format("%H:%M:%S %Z");
        let color = event.severity.color_code();
        let reset = "\x1b[0m";

        println!(
            "[{}] {}{:8}{} [{}] {}",
            time_str,
            color,
            format!("{:?}", event.severity).to_uppercase(),
            reset,
            event.source,
            event.message
        );

        if let Some(ref metadata) = event.metadata {
            if let Some(obj) = metadata.as_object() {
                for (key, value) in obj.iter().take(3) {
                    println!("         {} {}: {}", "│", key, value);
                }
            }
        }
    }

    println!("━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━");
    println!("Total: {} events", events.len());
}

fn print_events_compact(events: &[Event]) {
    for event in events {
        // Convert UTC to local timezone (respects TZ environment variable)
        let local_time = event.timestamp.with_timezone(&Local);
        let time_str = local_time.format("%H:%M:%S %Z");
        println!(
            "[{}] [{:8}] [{}] {}",
            time_str,
            format!("{:?}", event.severity),
            event.source,
            event.message
        );
    }
}

fn handle_subscribe(
    _follow: bool,
    _source: Option<String>,
    _severity: Option<String>,
) -> Result<()> {
    // TODO: Implement live streaming
    eprintln!("Subscribe command not yet implemented");
    eprintln!("Use 'nabi events pull --since=5m' and run in a loop for now");
    Ok(())
}

fn handle_cleanup(dry_run: bool, _force: bool) -> Result<()> {
    let store_path = get_event_store_path()?;

    if !store_path.exists() {
        println!("No events to clean up");
        return Ok(());
    }

    let file = File::open(&store_path).context("Failed to open event store")?;
    let reader = BufReader::new(file);

    let now = Utc::now();
    let mut events_to_keep = Vec::new();
    let mut events_to_delete = 0;

    for line in reader.lines() {
        let line = line?;
        let event: Event = match serde_json::from_str(&line) {
            Ok(e) => e,
            Err(_) => continue,
        };

        let retention_days = event.severity.retention_days();
        let cutoff = now - Duration::days(retention_days);

        if event.timestamp >= cutoff {
            events_to_keep.push(line);
        } else {
            events_to_delete += 1;
        }
    }

    if dry_run {
        println!("Would delete {} old events", events_to_delete);
        println!("Would keep {} events", events_to_keep.len());
    } else {
        // Write back only events to keep
        let mut file = File::create(&store_path)?;
        for line in events_to_keep {
            writeln!(file, "{}", line)?;
        }
        println!("Deleted {} old events", events_to_delete);
    }

    Ok(())
}

fn handle_stats(format: OutputFormat) -> Result<()> {
    let store_path = get_event_store_path()?;

    if !store_path.exists() {
        println!("No events found");
        return Ok(());
    }

    let file = File::open(&store_path)?;
    let reader = BufReader::new(file);

    let mut total = 0;
    let mut by_source: std::collections::HashMap<String, usize> = std::collections::HashMap::new();
    let mut by_severity: std::collections::HashMap<String, usize> =
        std::collections::HashMap::new();

    for line in reader.lines() {
        let line = line?;
        let event: Event = match serde_json::from_str(&line) {
            Ok(e) => e,
            Err(_) => continue,
        };

        total += 1;
        *by_source.entry(event.source.clone()).or_insert(0) += 1;
        *by_severity
            .entry(format!("{:?}", event.severity))
            .or_insert(0) += 1;
    }

    match format {
        OutputFormat::Json => {
            let stats = serde_json::json!({
                "total": total,
                "by_source": by_source,
                "by_severity": by_severity,
            });
            println!("{}", serde_json::to_string_pretty(&stats)?);
        }
        _ => {
            println!("Event Bus Statistics");
            println!("━━━━━━━━━━━━━━━━━━━");
            println!("Total events: {}", total);
            println!("\nBy Source:");
            for (source, count) in by_source.iter() {
                println!("  {}: {}", source, count);
            }
            println!("\nBy Severity:");
            for (severity, count) in by_severity.iter() {
                println!("  {}: {}", severity, count);
            }
        }
    }

    Ok(())
}
