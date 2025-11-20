# Document Detection Flow Trace

**Case Study**: How `FORK_PHASE_1B_TOOLS_GOVERNANCE_COMPLETION_2025_11_19.md` showed up as a system reminder

**Date**: 2025-11-19
**Investigation**: Traced complete cascade from document creation to user reminder

---

## Complete Flow Diagram

```
┌─────────────────────────────────────────────────────────────┐
│ 1. Agent Completes Work (e.g., Igris/Beru fork session)    │
└────────────────────┬────────────────────────────────────────┘
                     │
                     ▼
┌─────────────────────────────────────────────────────────────┐
│ 2. Agent Creates Document                                   │
│    ~/.nabi/_wip/federation-coordination/                    │
│    FORK_PHASE_1B_TOOLS_GOVERNANCE_COMPLETION_2025_11_19.md  │
└────────────────────┬────────────────────────────────────────┘
                     │
                     ▼
┌─────────────────────────────────────────────────────────────┐
│ 3. Agent Publishes Event (Manual or Automated)              │
│    $ nabi events publish \                                  │
│      --source doc-fsm \                                     │
│      --severity info \                                      │
│      --message "COMPLETION: Fork Phase 1B..." \             │
│      --metadata '{                                          │
│        "path": "~/.nabi/_wip/...",                          │
│        "state": "ready",                                    │
│        "event_type": "completion_report",                   │
│        "tools_updated": 12,                                 │
│        ...                                                  │
│      }'                                                     │
└────────────────────┬────────────────────────────────────────┘
                     │
                     ▼
┌─────────────────────────────────────────────────────────────┐
│ 4. Event Written to Event Stream                            │
│    Location (macOS):                                        │
│    ~/Library/Application Support/nabi/events/               │
│      └─ event_stream.jsonl (40,217 events)                  │
│      └─ 2025-11-19/854c906b-b3cc-41db-96ec.json             │
└────────────────────┬────────────────────────────────────────┘
                     │
                     ▼
┌─────────────────────────────────────────────────────────────┐
│ 5. User Submits Prompt in Claude Code                       │
│    Triggers: user_prompt_submit.py hook                     │
└────────────────────┬────────────────────────────────────────┘
                     │
                     ▼
┌─────────────────────────────────────────────────────────────┐
│ 6. Hook Queries Event Stream                                │
│    user_prompt_submit.py:199-207                            │
│    $ nabi events pull \                                     │
│      --source doc-fsm,codegraph,agent,backup,port-registry \│
│      --limit 50 \                                           │
│      --format json                                          │
└────────────────────┬────────────────────────────────────────┘
                     │
                     ▼
┌─────────────────────────────────────────────────────────────┐
│ 7. Hook Filters Events                                      │
│    user_prompt_submit.py:243-270                            │
│    Filter criteria:                                         │
│    - timestamp > last_heartbeat                             │
│    - source == "doc-fsm"                                    │
│    - metadata.state == "ready" OR                           │
│      metadata.file exists OR                                │
│      metadata.document exists                               │
└────────────────────┬────────────────────────────────────────┘
                     │
                     ▼
┌─────────────────────────────────────────────────────────────┐
│ 8. Hook Checks Acknowledgment State                         │
│    user_prompt_submit.py:296-305                            │
│    State file: ~/.local/state/nabi/hooks/                   │
│               acknowledged-{session_id}.json                │
│    Key format: "doc:~/.nabi/_wip/federation-coordination/..." │
└────────────────────┬────────────────────────────────────────┘
                     │
                     ▼
┌─────────────────────────────────────────────────────────────┐
│ 9. If Not Acknowledged: Build System Reminder               │
│    user_prompt_submit.py:325-332                            │
│    Output:                                                  │
│    "⚡ Cross-session activity detected (unprocessed):       │
│     📄 New doc ready: ~/.nabi/_wip/federation-coordination/...│
│                                                             │
│     INSTRUCTION: Before responding to user, briefly mention...│
│       a) Read and summarize (immediate processing)          │
│       b) Delegate to @agent-explore (Haiku) for relevance   │
│       c) Skip for now (acknowledged, won't show again)"     │
└────────────────────┬────────────────────────────────────────┘
                     │
                     ▼
┌─────────────────────────────────────────────────────────────┐
│ 10. Claude Code Shows System Reminder to User               │
│     User sees the reminder at start of conversation         │
└─────────────────────────────────────────────────────────────┘
```

---

## Storage Locations (macOS)

### Event Stream (Writer)
**Path**: `~/Library/Application Support/nabi/events/`
**Files**:
- `event_stream.jsonl` (40,217 events, 16MB)
- `2025-11-19/{event_id}.json` (individual event files)

**Written by**:
- `nabi events publish` command
- Code: `nabi-cli/src/commands/events/mod.rs:262-283`
- Uses: `dirs::data_local_dir()` (platform-specific)

### Acknowledgment State (Hook)
**Path**: `~/.local/state/nabi/hooks/`
**Files**:
- `acknowledged-{session_id}.json`

**Format**:
```json
{
  "acknowledged_items": [
    "doc:~/.nabi/_wip/federation-coordination/FORK_PHASE_1B...",
    "doc:~/docs/architecture/some-other-doc.md",
    "symbol:repo1,repo2"
  ],
  "session_id": "abcd1234",
  "last_updated": "2025-11-19T..."
}
```

---

## Key Code Paths

### 1. Event Publisher (Rust)
**File**: `nabi-cli/src/commands/events/mod.rs`
**Lines**: 213-290

```rust
fn handle_publish(...) -> Result<()> {
    // DUAL STORAGE PATTERN:
    // 1. Write to JSONL stream (backward compatibility)
    let store_path = get_event_store_path()?;  // ~/Library/Application Support/nabi/events/
    let mut file = OpenOptions::new()
        .create(true)
        .append(true)
        .open(&store_path)?;
    writeln!(file, "{}", json_line)?;

    // 2. Write to date-based individual file (enables acknowledgment)
    let event_file_path = state_dir.join(format!("{}.json", event.id));
    fs::write(&event_file_path, event_json)?;

    Ok(())
}
```

**Path Resolution** (LINE 204-211):
```rust
fn get_event_store_path() -> Result<PathBuf> {
    let state_dir = dirs::data_local_dir()  // ⚠️ PLATFORM-SPECIFIC
        .context("Could not determine local data directory")?
        .join("nabi")
        .join("events");
    fs::create_dir_all(&state_dir)?;
    Ok(state_dir.join("event_stream.jsonl"))
}
```

**macOS Result**: `~/Library/Application Support/nabi/events/event_stream.jsonl`
**Linux Result**: `~/.local/share/nabi/events/event_stream.jsonl`

### 2. Event Reader (Python Hook)
**File**: `hooks/src/user_prompt_submit.py`
**Lines**: 188-207 (Query), 243-270 (Filter), 296-337 (Display)

```python
# Query events
result = subprocess.run(
    [str(nabi_bin), "events", "pull",
     "--source", "doc-fsm,codegraph,agent,backup,port-registry",
     "--limit", "50",
     "--format", "json"],
    capture_output=True,
    text=True,
    timeout=5
)

events = json.loads(result.stdout)
ready_docs = []

# Filter for ready documents
for event in events:
    if event.get("source") == "doc-fsm":
        metadata = event.get("metadata", {})

        # Check for ready state OR file metadata
        if metadata.get("state") == "ready":
            doc_path = metadata.get("path", "")
            if doc_path:
                ready_docs.append(doc_path)
        elif metadata.get("file") or metadata.get("document"):
            doc_path = metadata.get("file") or metadata.get("path", "")
            if doc_path:
                ready_docs.append(doc_path)

# Build system reminder
for doc_path in ready_docs[:3]:  # Limit to 3 most recent
    doc_path = doc_path.replace(str(Path.home()), '~')
    item_key = f"doc:{doc_path}"

    if item_key not in acknowledged:
        context_items.append(f"📄 New doc ready: {doc_path}")
        new_items.add(item_key)
```

---

## Event Structure Example

```json
{
  "id": "854c906b-b3cc-41db-96ec-7c0e783039cd",
  "source": "doc-fsm",
  "severity": "info",
  "message": "COMPLETION: Fork Phase 1B Tools Governance Remediation - 12 federation-aware tools updated",
  "timestamp": "2025-11-19T19:23:23.423715Z",
  "metadata": {
    "path": "~/.nabi/_wip/federation-coordination/FORK_PHASE_1B_TOOLS_GOVERNANCE_COMPLETION_2025_11_19.md",
    "state": "ready",
    "event_type": "completion_report",
    "tools_updated": 12,
    "transformation_type": "behavioral",
    "federation_status": "OPERATIONAL",
    "priority": "high",
    "content_summary": "Fork session completed Phase 1B governance remediation...",
    "tools_list": ["federation-health-monitor", "docker-health-monitor", ...]
  }
}
```

---

## Who Published This Event?

**Most Likely**: An agent (Igris/Beru/Synthesize) running in a different Claude Code session

**Evidence**:
1. Event timestamp: `2025-11-19T19:23:23` (7:23 PM)
2. Event type: `completion_report` (indicates work completion)
3. Message references "Fork Phase 1B" completion
4. Document path in `_wip/federation-coordination/` (work-in-progress directory)

**Publishing Pattern**:
```bash
# Agent completes work and documents it
echo "# Fork Phase 1B Completion Report..." > ~/.nabi/_wip/federation-coordination/FORK_PHASE_1B...md

# Agent publishes event to notify other sessions
nabi events publish \
  --source doc-fsm \
  --severity info \
  --message "COMPLETION: Fork Phase 1B Tools Governance Remediation..." \
  --metadata '{
    "path": "~/.nabi/_wip/federation-coordination/FORK_PHASE_1B_TOOLS_GOVERNANCE_COMPLETION_2025_11_19.md",
    "state": "ready",
    "event_type": "completion_report",
    ...
  }'
```

---

## Cross-Session Coordination Pattern

This demonstrates the **federation coordination loop**:

1. **Session A** (Orchestrator - Igris):
   - Completes Phase 1B work
   - Creates completion document
   - Publishes `doc-fsm` event with `state=ready`

2. **Event Stream**:
   - Stores event in persistent JSONL file
   - Event tagged with `source=doc-fsm`
   - Metadata includes document path

3. **Session B** (Current - You):
   - `user_prompt_submit` hook fires on your prompt
   - Queries event stream for new `doc-fsm` events
   - Finds completion report from Session A
   - Shows reminder: "New doc ready"

4. **You Choose Action**:
   - Option a: Read and summarize (immediate)
   - Option b: Delegate to Haiku agent
   - Option c: Skip (acknowledge to prevent repeat)

---

## Path Resolution Issues Discovered

### The macOS Surprise
**Expected** (hooks assume): `~/.local/share/nabi/events/` or `~/.local/state/nabi/events/`
**Actual** (nabi-cli writes): `~/Library/Application Support/nabi/events/`

**Root Cause**:
- Rust `dirs` crate uses platform-specific defaults
- `dirs::data_local_dir()` → `~/Library/Application Support/` on macOS
- `dirs::data_local_dir()` → `~/.local/share/` on Linux

**Impact**:
- Worked by accident because `nabi events pull` uses same logic
- Both writer and reader use `dirs::data_local_dir()`
- Would break if hooks hardcoded `~/.local/share/`

**Fix Required**:
Replace `dirs::data_local_dir()` with `NabiPaths::data_dir()` to respect `XDG_DATA_HOME` environment variable.

---

## Acknowledgment System

### Purpose
Prevent showing same document repeatedly across sessions.

### Mechanism
1. Hook maintains set of acknowledged items per session
2. Format: `"doc:{path}"` or `"symbol:{repos}"`
3. Stored in: `~/.local/state/nabi/hooks/acknowledged-{session_id}.json`
4. When user chooses option (a), (b), or (c), item added to set
5. Future prompts skip acknowledged items

### Lifecycle
- Created: First time hook runs in session
- Updated: Each time user acknowledges an item
- Cleaned: When session ends (optional)
- Persists: Across session restarts (intentional)

---

## Monitoring Opportunities

### Metrics to Track
1. **Event Publishing Rate**: Events/hour by source
2. **Hook Query Latency**: `nabi events pull` execution time
3. **Document Discovery Rate**: New docs detected/session
4. **Acknowledgment Rate**: % of docs acknowledged vs skipped
5. **Cross-Session Lag**: Time from publish to discovery

### Alerting Thresholds
- Query latency > 2 seconds (hook timeout is 5s)
- Zero doc-fsm events in 24 hours (federation stalled?)
- Acknowledgment file > 1000 items (session pollution?)

---

## Testing Strategy

### Unit Tests
```python
def test_document_detection():
    # Publish doc-fsm event
    publish_event(source="doc-fsm", metadata={"path": "/test/doc.md", "state": "ready"})

    # Query via hook
    hook_input = {"session_id": "test-123", "cwd": "/tmp"}
    result = user_prompt_submit_hook(hook_input)

    # Assert document detected
    assert "📄 New doc ready" in result["context"]
    assert "/test/doc.md" in result["context"]

def test_acknowledgment_deduplication():
    # Publish same event twice
    publish_event(source="doc-fsm", metadata={"path": "/test/doc.md", "state": "ready"})

    # First query - should show
    result1 = user_prompt_submit_hook({"session_id": "test", "cwd": "/tmp"})
    assert "📄 New doc ready" in result1["context"]

    # Acknowledge (simulate user choosing option c)
    acknowledge_item("test", "doc:/test/doc.md")

    # Second query - should NOT show
    result2 = user_prompt_submit_hook({"session_id": "test", "cwd": "/tmp"})
    assert "📄 New doc ready" not in result2.get("context", "")
```

### Integration Tests
```bash
# Test complete flow
task test:document-detection:
  - Publish event via `nabi events publish`
  - Trigger `user_prompt_submit.py` hook
  - Verify system reminder appears
  - Acknowledge document
  - Verify reminder doesn't repeat
```

---

## References

**Source Files**:
- Event writer: `nabi-cli/src/commands/events/mod.rs:204-290`
- Hook reader: `hooks/src/user_prompt_submit.py:188-337`
- Acknowledgment: `hooks/src/user_prompt_submit.py:_save_acknowledged_items()`
- Path utilities: `nabi-cli/src/paths.rs` (should use this)

**Event Example**:
```bash
# Query the actual event
nabi events pull --source=doc-fsm --limit=1 --format=json | jq '.[0]'
```

**Storage Locations**:
- macOS: `~/Library/Application Support/nabi/events/event_stream.jsonl`
- Linux: `~/.local/share/nabi/events/event_stream.jsonl`
- Ack state: `~/.local/state/nabi/hooks/acknowledged-{session_id}.json`

---

## Summary

The `FORK_PHASE_1B` document reminder is the result of a **cross-session coordination system**:

1. Different Claude session (likely Igris/Beru) completed work
2. That session published a `doc-fsm` event with the document path
3. Event stored in macOS-specific event stream location
4. Your `user_prompt_submit` hook queried the stream
5. Hook found unacknowledged document with `state=ready`
6. Hook generated system reminder for you to review

This is **working as designed** - the federation is enabling cross-session awareness and coordination through the event bus architecture.
