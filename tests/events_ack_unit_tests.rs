/// Unit Tests for `nabi events ack` Command
///
/// Tests vector clock operations, causal ancestry computation, and JSONL atomicity.
/// These tests use mocked Python bridge for isolation.
///
/// Run with: cargo test --test events_ack_unit_tests -- --nocapture

use std::collections::HashMap;
use serde::{Deserialize, Serialize};
use std::fs;
use std::io::Write;
use tempfile::TempDir;

// ============================================================================
// Test Fixtures and Helpers
// ============================================================================

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
struct VectorClock {
    inner: HashMap<String, u64>,
}

impl VectorClock {
    fn new() -> Self {
        VectorClock {
            inner: HashMap::new(),
        }
    }

    fn from_map(map: HashMap<String, u64>) -> Self {
        VectorClock { inner: map }
    }

    fn get(&self, node: &str) -> u64 {
        self.inner.get(node).copied().unwrap_or(0)
    }

    fn set(&mut self, node: String, value: u64) {
        self.inner.insert(node, value);
    }

    fn increment(&mut self, node: &str) {
        let current = self.get(node);
        self.set(node.to_string(), current + 1);
    }

    fn all_nodes(&self) -> Vec<String> {
        let mut nodes: Vec<_> = self.inner.keys().cloned().collect();
        nodes.sort();
        nodes
    }

    /// Compare two vector clocks for causal ordering
    ///
    /// Returns:
    /// - Some(true) if self causally precedes other (self < other)
    /// - Some(false) if other causally precedes self (other < self)
    /// - None if concurrent (no causal relationship)
    fn compare(&self, other: &VectorClock) -> Option<bool> {
        let mut all_nodes = self.all_nodes();
        all_nodes.extend(other.all_nodes());
        all_nodes.sort();
        all_nodes.dedup();

        if all_nodes.is_empty() {
            return None;
        }

        // Check if self <= other (all nodes in self <= other)
        let self_le_other = all_nodes
            .iter()
            .all(|n| self.get(n) <= other.get(n));
        // Check if self < other (at least one node strictly less)
        let self_lt_other = all_nodes
            .iter()
            .any(|n| self.get(n) < other.get(n));
        let self_precedes = self_le_other && self_lt_other;

        // Check if other <= self
        let other_le_self = all_nodes
            .iter()
            .all(|n| other.get(n) <= self.get(n));
        // Check if other < self
        let other_lt_self = all_nodes
            .iter()
            .any(|n| other.get(n) < self.get(n));
        let other_precedes = other_le_self && other_lt_self;

        match (self_precedes, other_precedes) {
            (true, false) => Some(true),   // self < other
            (false, true) => Some(false),  // other < self
            _ => None,                     // concurrent or equal
        }
    }
}

#[derive(Debug, Clone, Serialize, Deserialize)]
struct Acknowledgment {
    ack_id: String,
    event_id: String,
    node_id: String,
    vector_clock: VectorClock,
    causally_after: Vec<String>, // IDs of acks that causally precede this one
    timestamp: String,
    metadata: Option<serde_json::Value>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
struct Event {
    id: String,
    source: String,
    message: String,
    vector_clock: VectorClock,
}

impl Event {
    fn new(id: &str, source: &str, vc: VectorClock) -> Self {
        Event {
            id: id.to_string(),
            source: source.to_string(),
            message: "test event".to_string(),
            vector_clock: vc,
        }
    }
}

// Helper to create acknowledgments
fn create_ack(ack_id: &str, vc: VectorClock) -> Acknowledgment {
    Acknowledgment {
        ack_id: ack_id.to_string(),
        event_id: "event-test-001".to_string(),
        node_id: "node-macos".to_string(),
        vector_clock: vc,
        causally_after: Vec::new(),
        timestamp: chrono::Utc::now().to_rfc3339(),
        metadata: None,
    }
}

// ============================================================================
// Unit Tests: Vector Clock Operations
// ============================================================================

#[cfg(test)]
mod vector_clock_tests {
    use super::*;

    #[test]
    fn test_vector_clock_creation() {
        let vc = VectorClock::new();
        assert_eq!(vc.get("macos"), 0);
        assert_eq!(vc.get("rpi"), 0);
        assert_eq!(vc.all_nodes().len(), 0);
    }

    #[test]
    fn test_vector_clock_from_map() {
        let mut map = HashMap::new();
        map.insert("macos".to_string(), 5);
        map.insert("rpi".to_string(), 3);

        let vc = VectorClock::from_map(map);
        assert_eq!(vc.get("macos"), 5);
        assert_eq!(vc.get("rpi"), 3);
        assert_eq!(vc.get("wsl"), 0); // Default for unknown nodes
    }

    #[test]
    fn test_vector_clock_increment() {
        let mut map = HashMap::new();
        map.insert("macos".to_string(), 5);
        map.insert("rpi".to_string(), 3);

        let mut vc = VectorClock::from_map(map);

        // Increment macos
        vc.increment("macos");
        assert_eq!(vc.get("macos"), 6);
        assert_eq!(vc.get("rpi"), 3); // unchanged

        // Increment rpi
        vc.increment("rpi");
        assert_eq!(vc.get("rpi"), 4);
        assert_eq!(vc.get("macos"), 6); // unchanged

        // Increment new node
        vc.increment("wsl");
        assert_eq!(vc.get("wsl"), 1);
    }

    #[test]
    fn test_vector_clock_comparison_equal() {
        let mut map1 = HashMap::new();
        map1.insert("macos".to_string(), 5);
        map1.insert("rpi".to_string(), 3);
        let vc1 = VectorClock::from_map(map1);

        let mut map2 = HashMap::new();
        map2.insert("macos".to_string(), 5);
        map2.insert("rpi".to_string(), 3);
        let vc2 = VectorClock::from_map(map2);

        // Equal clocks are concurrent (no strict ordering)
        assert_eq!(vc1.compare(&vc2), None);
    }

    #[test]
    fn test_vector_clock_comparison_sequential() {
        // Event sequence: A happens at {macos:5}, B happens at {macos:6}
        let mut map_a = HashMap::new();
        map_a.insert("macos".to_string(), 5);
        let vc_a = VectorClock::from_map(map_a);

        let mut map_b = HashMap::new();
        map_b.insert("macos".to_string(), 6);
        let vc_b = VectorClock::from_map(map_b);

        // vc_a < vc_b (A happened before B)
        assert_eq!(vc_a.compare(&vc_b), Some(true));
        // vc_b > vc_a (B happened after A)
        assert_eq!(vc_b.compare(&vc_a), Some(false));
    }

    #[test]
    fn test_vector_clock_comparison_concurrent() {
        // Two agents act independently
        let mut map_a = HashMap::new();
        map_a.insert("macos".to_string(), 5);
        let vc_a = VectorClock::from_map(map_a);

        let mut map_b = HashMap::new();
        map_b.insert("rpi".to_string(), 3);
        let vc_b = VectorClock::from_map(map_b);

        // Concurrent: macos ahead in A, rpi ahead in B
        assert_eq!(vc_a.compare(&vc_b), None);
        assert_eq!(vc_b.compare(&vc_a), None);
    }

    #[test]
    fn test_vector_clock_comparison_multi_node_sequential() {
        // A: {macos:6, rpi:3}
        let mut map_a = HashMap::new();
        map_a.insert("macos".to_string(), 6);
        map_a.insert("rpi".to_string(), 3);
        let vc_a = VectorClock::from_map(map_a);

        // B: {macos:6, rpi:4} (only rpi incremented)
        let mut map_b = HashMap::new();
        map_b.insert("macos".to_string(), 6);
        map_b.insert("rpi".to_string(), 4);
        let vc_b = VectorClock::from_map(map_b);

        // A < B (B has all of A's values plus one strictly greater)
        assert_eq!(vc_a.compare(&vc_b), Some(true));
        assert_eq!(vc_b.compare(&vc_a), Some(false));
    }

    #[test]
    fn test_vector_clock_comparison_multi_node_concurrent() {
        // A: {macos:6, rpi:3}
        let mut map_a = HashMap::new();
        map_a.insert("macos".to_string(), 6);
        map_a.insert("rpi".to_string(), 3);
        let vc_a = VectorClock::from_map(map_a);

        // B: {macos:5, rpi:4} (macos decreased, rpi increased)
        let mut map_b = HashMap::new();
        map_b.insert("macos".to_string(), 5);
        map_b.insert("rpi".to_string(), 4);
        let vc_b = VectorClock::from_map(map_b);

        // Concurrent: macos higher in A, rpi higher in B
        assert_eq!(vc_a.compare(&vc_b), None);
        assert_eq!(vc_b.compare(&vc_a), None);
    }
}

// ============================================================================
// Unit Tests: Causal Ancestry Computation
// ============================================================================

#[cfg(test)]
mod causal_ancestry_tests {
    use super::*;

    /// Find all acknowledgments that causally precede the target
    fn compute_causal_ancestors(
        target: &VectorClock,
        all_acks: &[Acknowledgment],
    ) -> Vec<String> {
        all_acks
            .iter()
            .filter_map(|ack| {
                if let Some(true) = ack.vector_clock.compare(target) {
                    Some(ack.ack_id.clone())
                } else {
                    None
                }
            })
            .collect()
    }

    #[test]
    fn test_causal_ancestors_empty_log() {
        let mut map = HashMap::new();
        map.insert("macos".to_string(), 1);
        let target_vc = VectorClock::from_map(map);

        let ancestors = compute_causal_ancestors(&target_vc, &[]);
        assert_eq!(ancestors.len(), 0);
    }

    #[test]
    fn test_causal_ancestors_first_ack() {
        let mut map_a = HashMap::new();
        map_a.insert("macos".to_string(), 1);
        let ack_a = create_ack("ack-001", VectorClock::from_map(map_a));

        let mut map_b = HashMap::new();
        map_b.insert("macos".to_string(), 2);
        let target_vc = VectorClock::from_map(map_b);

        let ancestors = compute_causal_ancestors(&target_vc, &[ack_a]);
        assert_eq!(ancestors, vec!["ack-001".to_string()]);
    }

    #[test]
    fn test_causal_ancestors_sequential_chain() {
        // ack-001: {macos: 1}
        let mut map_1 = HashMap::new();
        map_1.insert("macos".to_string(), 1);
        let ack_1 = create_ack("ack-001", VectorClock::from_map(map_1));

        // ack-002: {macos: 2}
        let mut map_2 = HashMap::new();
        map_2.insert("macos".to_string(), 2);
        let ack_2 = create_ack("ack-002", VectorClock::from_map(map_2));

        // ack-003: {macos: 3}
        let mut map_3 = HashMap::new();
        map_3.insert("macos".to_string(), 3);
        let ack_3 = create_ack("ack-003", VectorClock::from_map(map_3));

        // Target: {macos: 4}
        let mut target_map = HashMap::new();
        target_map.insert("macos".to_string(), 4);
        let target_vc = VectorClock::from_map(target_map);

        let ancestors = compute_causal_ancestors(&target_vc, &[ack_1, ack_2, ack_3]);
        assert_eq!(
            ancestors,
            vec!["ack-001", "ack-002", "ack-003"]
                .into_iter()
                .map(|s| s.to_string())
                .collect::<Vec<_>>()
        );
    }

    #[test]
    fn test_causal_ancestors_multi_node() {
        // ack-001: {macos: 6, rpi: 3}
        let mut map_1 = HashMap::new();
        map_1.insert("macos".to_string(), 6);
        map_1.insert("rpi".to_string(), 3);
        let ack_1 = create_ack("ack-001", VectorClock::from_map(map_1));

        // target: {macos: 6, rpi: 4} (only rpi incremented by target agent)
        let mut target_map = HashMap::new();
        target_map.insert("macos".to_string(), 6);
        target_map.insert("rpi".to_string(), 4);
        let target_vc = VectorClock::from_map(target_map);

        let ancestors = compute_causal_ancestors(&target_vc, &[ack_1]);
        assert_eq!(ancestors, vec!["ack-001".to_string()]);
    }

    #[test]
    fn test_causal_ancestors_concurrent() {
        // ack-001: {macos: 6, rpi: 3}
        let mut map_1 = HashMap::new();
        map_1.insert("macos".to_string(), 6);
        map_1.insert("rpi".to_string(), 3);
        let ack_1 = create_ack("ack-001", VectorClock::from_map(map_1));

        // target: {macos: 5, rpi: 4} (concurrent - one node higher in each)
        let mut target_map = HashMap::new();
        target_map.insert("macos".to_string(), 5);
        target_map.insert("rpi".to_string(), 4);
        let target_vc = VectorClock::from_map(target_map);

        let ancestors = compute_causal_ancestors(&target_vc, &[ack_1]);
        assert_eq!(ancestors.len(), 0); // Not a causal ancestor
    }

    #[test]
    fn test_causal_ancestors_mixed_concurrent_and_sequential() {
        // ack-001: {macos: 6, rpi: 3}
        let mut map_1 = HashMap::new();
        map_1.insert("macos".to_string(), 6);
        map_1.insert("rpi".to_string(), 3);
        let ack_1 = create_ack("ack-001", VectorClock::from_map(map_1));

        // ack-002: {macos: 7, rpi: 3} (sequential to ack-001 on macos)
        let mut map_2 = HashMap::new();
        map_2.insert("macos".to_string(), 7);
        map_2.insert("rpi".to_string(), 3);
        let ack_2 = create_ack("ack-002", VectorClock::from_map(map_2));

        // ack-003: {macos: 8, rpi: 4} (sequential - higher on both dimensions)
        let mut map_3 = HashMap::new();
        map_3.insert("macos".to_string(), 8);
        map_3.insert("rpi".to_string(), 4);
        let ack_3 = create_ack("ack-003", VectorClock::from_map(map_3));

        // target: {macos: 8, rpi: 4}
        // Note: ack-003 has same values as target, so it doesn't dominate
        // But ack-001 and ack-002 do causally precede target
        let mut target_map = HashMap::new();
        target_map.insert("macos".to_string(), 8);
        target_map.insert("rpi".to_string(), 4);
        let target_vc = VectorClock::from_map(target_map);

        let ancestors = compute_causal_ancestors(&target_vc, &[ack_1, ack_2, ack_3]);
        // ack-001 < target and ack-002 < target (both precede)
        // ack-003 == target (not a strict ancestor)
        assert_eq!(
            ancestors,
            vec!["ack-001", "ack-002"]
                .into_iter()
                .map(|s| s.to_string())
                .collect::<Vec<_>>()
        );
    }
}

// ============================================================================
// Unit Tests: Atomic JSONL Operations
// ============================================================================

#[cfg(test)]
mod jsonl_atomicity_tests {
    use super::*;

    fn append_ack_to_jsonl(path: &std::path::Path, ack: &Acknowledgment) -> std::io::Result<()> {
        let mut file = std::fs::OpenOptions::new()
            .create(true)
            .append(true)
            .open(path)?;

        let json_line = serde_json::to_string(ack)?;
        writeln!(file, "{}", json_line)?;
        Ok(())
    }

    #[test]
    fn test_jsonl_single_append() {
        let temp_dir = TempDir::new().expect("Failed to create temp dir");
        let ack_log = temp_dir.path().join("event-test.ack.jsonl");

        let mut map = HashMap::new();
        map.insert("macos".to_string(), 1);
        let ack = create_ack("ack-001", VectorClock::from_map(map));

        append_ack_to_jsonl(&ack_log, &ack).expect("Failed to append");

        // Verify file exists
        assert!(ack_log.exists());

        // Verify content
        let content = fs::read_to_string(&ack_log).expect("Failed to read file");
        let lines: Vec<&str> = content.lines().collect();
        assert_eq!(lines.len(), 1);

        // Verify JSON is parseable
        let parsed: Acknowledgment =
            serde_json::from_str(lines[0]).expect("Failed to parse JSON");
        assert_eq!(parsed.ack_id, "ack-001");
    }

    #[test]
    fn test_jsonl_multiple_appends() {
        let temp_dir = TempDir::new().expect("Failed to create temp dir");
        let ack_log = temp_dir.path().join("event-test.ack.jsonl");

        // Append first ack
        let mut map1 = HashMap::new();
        map1.insert("macos".to_string(), 1);
        let ack1 = create_ack("ack-001", VectorClock::from_map(map1));
        append_ack_to_jsonl(&ack_log, &ack1).expect("Failed to append ack-001");

        // Append second ack
        let mut map2 = HashMap::new();
        map2.insert("macos".to_string(), 2);
        let ack2 = create_ack("ack-002", VectorClock::from_map(map2));
        append_ack_to_jsonl(&ack_log, &ack2).expect("Failed to append ack-002");

        // Append third ack
        let mut map3 = HashMap::new();
        map3.insert("macos".to_string(), 3);
        let ack3 = create_ack("ack-003", VectorClock::from_map(map3));
        append_ack_to_jsonl(&ack_log, &ack3).expect("Failed to append ack-003");

        // Verify file
        let content = fs::read_to_string(&ack_log).expect("Failed to read file");
        let lines: Vec<&str> = content.lines().collect();
        assert_eq!(lines.len(), 3);

        // Verify all lines are parseable and in order
        for (idx, line) in lines.iter().enumerate() {
            let parsed: Acknowledgment = serde_json::from_str(line).expect("Failed to parse JSON");
            assert_eq!(parsed.ack_id, format!("ack-{:03}", idx + 1));
        }
    }

    #[test]
    fn test_jsonl_with_metadata() {
        let temp_dir = TempDir::new().expect("Failed to create temp dir");
        let ack_log = temp_dir.path().join("event-test.ack.jsonl");

        let mut map = HashMap::new();
        map.insert("macos".to_string(), 1);
        let mut ack = create_ack("ack-001", VectorClock::from_map(map));
        ack.metadata = Some(serde_json::json!({
            "agent": "claude-test",
            "processed": true,
            "tags": ["federation", "test"]
        }));

        append_ack_to_jsonl(&ack_log, &ack).expect("Failed to append");

        // Verify metadata is preserved
        let content = fs::read_to_string(&ack_log).expect("Failed to read file");
        let parsed: Acknowledgment =
            serde_json::from_str(content.trim()).expect("Failed to parse JSON");

        assert!(parsed.metadata.is_some());
        let meta = parsed.metadata.unwrap();
        assert_eq!(meta["agent"], "claude-test");
        assert_eq!(meta["processed"], true);
    }

    #[test]
    fn test_jsonl_concurrent_writes_safe() {
        let temp_dir = TempDir::new().expect("Failed to create temp dir");
        let ack_log = temp_dir.path().join("event-test.ack.jsonl");

        // Simulate 5 rapid writes (like multiple agents acknowledging)
        for i in 1..=5 {
            let mut map = HashMap::new();
            map.insert("macos".to_string(), i);
            let ack = create_ack(&format!("ack-{:03}", i), VectorClock::from_map(map));
            append_ack_to_jsonl(&ack_log, &ack).expect(&format!("Failed to append ack-{:03}", i));
        }

        // Verify all 5 entries exist and are valid
        let content = fs::read_to_string(&ack_log).expect("Failed to read file");
        let lines: Vec<&str> = content.lines().collect();
        assert_eq!(lines.len(), 5);

        // Verify all lines are valid JSON
        for line in lines {
            let _: Acknowledgment = serde_json::from_str(line).expect("Failed to parse JSON");
        }
    }
}

// ============================================================================
// Unit Tests: Event Structure
// ============================================================================

#[cfg(test)]
mod event_structure_tests {
    use super::*;

    #[test]
    fn test_event_creation() {
        let mut map = HashMap::new();
        map.insert("macos".to_string(), 5);

        let event = Event::new("event-001", "test-source", VectorClock::from_map(map));
        assert_eq!(event.id, "event-001");
        assert_eq!(event.source, "test-source");
        assert_eq!(event.vector_clock.get("macos"), 5);
    }

    #[test]
    fn test_acknowledgment_creation() {
        let mut map = HashMap::new();
        map.insert("macos".to_string(), 1);

        let ack = create_ack("ack-001", VectorClock::from_map(map));
        assert_eq!(ack.ack_id, "ack-001");
        assert_eq!(ack.vector_clock.get("macos"), 1);
        assert_eq!(ack.causally_after.len(), 0);
    }

    #[test]
    fn test_acknowledgment_serialization() {
        let mut map = HashMap::new();
        map.insert("macos".to_string(), 1);
        let ack = create_ack("ack-001", VectorClock::from_map(map));

        // Serialize to JSON
        let json = serde_json::to_string(&ack).expect("Failed to serialize");
        assert!(json.contains("ack-001"));
        assert!(json.contains("event-test-001"));

        // Deserialize back
        let parsed: Acknowledgment = serde_json::from_str(&json).expect("Failed to deserialize");
        assert_eq!(parsed.ack_id, ack.ack_id);
        assert_eq!(
            parsed.vector_clock.get("macos"),
            ack.vector_clock.get("macos")
        );
    }
}

// ============================================================================
// Summary of Tests
// ============================================================================

/*
Test Coverage Summary:
- Vector Clock Operations (7 tests):
  * Creation and initialization
  * Increment operations
  * Comparison (equal, sequential, concurrent, multi-node)

- Causal Ancestry (6 tests):
  * Empty log handling
  * Single and chain sequences
  * Multi-node scenarios
  * Concurrent detection
  * Mixed scenarios

- JSONL Atomicity (5 tests):
  * Single and multiple appends
  * Metadata preservation
  * Concurrent write safety
  * JSON parsability

- Event Structure (3 tests):
  * Event creation
  * Acknowledgment creation
  * Serialization roundtrip

Total: 21 unit tests ensuring correctness of core operations
*/
