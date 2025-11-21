/// Stream Events Command - Fast local event reading with filtering
/// Reads from event.jsonl with optional SurrealDB enrichment

use super::common::*;
use super::publish_event::McpEvent;
use anyhow::{Context, Result};
use chrono::{DateTime, Utc};
use serde::{Deserialize, Serialize};
use std::fs::File;
use std::io::{BufRead, BufReader};

#[derive(Debug, Serialize, Deserialize)]
pub struct StreamEventsOutput {
    pub events: Vec<McpEvent>,
    pub total: usize,
    pub filtered: usize,
    pub staleness_hint: String,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub enrichment_source: Option<String>,
}

/// Parse timestamp string to DateTime
fn parse_timestamp(ts: &str) -> Option<DateTime<Utc>> {
    DateTime::parse_from_rfc3339(ts)
        .ok()
        .map(|dt| dt.with_timezone(&Utc))
}

/// Filter events by source
fn filter_by_source(event: &McpEvent, source_filter: &Option<String>) -> bool {
    if let Some(ref filter) = source_filter {
        event.source == *filter
    } else {
        true
    }
}

/// Filter events by severity
fn filter_by_severity(event: &McpEvent, severity_filter: &Option<String>) -> bool {
    if let Some(ref filter) = severity_filter {
        event.severity == *filter
    } else {
        true
    }
}

/// Filter events by timestamp (after given timestamp)
fn filter_by_timestamp(event: &McpEvent, after_timestamp: &Option<String>) -> bool {
    if let Some(ref after) = after_timestamp {
        if let (Some(event_ts), Some(filter_ts)) =
            (parse_timestamp(&event.timestamp), parse_timestamp(after))
        {
            return event_ts > filter_ts;
        }
    }
    true
}

/// Try to enrich events from SurrealDB (non-blocking, <100ms timeout)
fn try_enrich_from_db(_events: &mut Vec<McpEvent>) -> Option<String> {
    // TODO: Implement SurrealDB enrichment with timeout
    // For now, return None to indicate no enrichment
    None
}

/// Execute stream_events command
pub fn execute(
    source: Option<String>,
    severity: Option<String>,
    after_timestamp: Option<String>,
    limit: Option<usize>,
) -> Result<()> {
    // Read from local event store
    let event_store_path = get_event_store_path()?;

    if !event_store_path.exists() {
        // Return empty result
        let output = StreamEventsOutput {
            events: vec![],
            total: 0,
            filtered: 0,
            staleness_hint: "no events".to_string(),
            enrichment_source: None,
        };
        println!("{}", serde_json::to_string_pretty(&output)?);
        return Ok(());
    }

    // Read and filter events
    let file = File::open(&event_store_path)
        .context("Failed to open event store")?;
    let reader = BufReader::new(file);

    let mut all_events = Vec::new();
    let mut total_count = 0;

    for line in reader.lines() {
        let line = line?;
        if line.trim().is_empty() {
            continue;
        }

        total_count += 1;

        // Try to parse as McpEvent
        let event: McpEvent = match serde_json::from_str(&line) {
            Ok(e) => e,
            Err(_) => continue, // Skip malformed lines
        };

        // Apply filters
        if !filter_by_source(&event, &source) {
            continue;
        }
        if !filter_by_severity(&event, &severity) {
            continue;
        }
        if !filter_by_timestamp(&event, &after_timestamp) {
            continue;
        }

        all_events.push(event);
    }

    // Apply limit and pagination
    let limit = limit.unwrap_or(50);
    let filtered_count = all_events.len();

    // Take most recent events (reverse to get newest first)
    all_events.reverse();
    all_events.truncate(limit);

    // Try to enrich from SurrealDB (best-effort, non-blocking)
    let enrichment_source = try_enrich_from_db(&mut all_events);

    // Calculate staleness hint
    let staleness_hint = if let Some(latest) = all_events.first() {
        if let Some(latest_ts) = parse_timestamp(&latest.timestamp) {
            let age = Utc::now().signed_duration_since(latest_ts);
            if age.num_seconds() < 60 {
                "fresh (< 1 minute)".to_string()
            } else if age.num_minutes() < 60 {
                format!("recent ({} minutes)", age.num_minutes())
            } else if age.num_hours() < 24 {
                format!("older ({} hours)", age.num_hours())
            } else {
                format!("stale ({} days)", age.num_days())
            }
        } else {
            "unknown".to_string()
        }
    } else {
        "no events".to_string()
    };

    // Build output
    let output = StreamEventsOutput {
        events: all_events,
        total: total_count,
        filtered: filtered_count,
        staleness_hint,
        enrichment_source,
    };

    // JSON output
    println!("{}", serde_json::to_string_pretty(&output)?);

    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_parse_timestamp() {
        let ts = "2025-11-19T12:34:56.789Z";
        assert!(parse_timestamp(ts).is_some());
    }

    #[test]
    fn test_filter_by_source() {
        let event = McpEvent {
            id: "evt_test".to_string(),
            timestamp: "2025-11-19T12:34:56.789Z".to_string(),
            source: "agent:igris".to_string(),
            severity: "info".to_string(),
            message: "test".to_string(),
            metadata: None,
            vector_clock: None,
        };

        assert!(filter_by_source(&event, &Some("agent:igris".to_string())));
        assert!(!filter_by_source(&event, &Some("agent:beru".to_string())));
        assert!(filter_by_source(&event, &None));
    }

    #[test]
    fn test_filter_by_severity() {
        let event = McpEvent {
            id: "evt_test".to_string(),
            timestamp: "2025-11-19T12:34:56.789Z".to_string(),
            source: "agent:igris".to_string(),
            severity: "info".to_string(),
            message: "test".to_string(),
            metadata: None,
            vector_clock: None,
        };

        assert!(filter_by_severity(&event, &Some("info".to_string())));
        assert!(!filter_by_severity(&event, &Some("critical".to_string())));
        assert!(filter_by_severity(&event, &None));
    }
}
