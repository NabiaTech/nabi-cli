# Race Condition Analysis: nabi tmux Commands

## Summary

The tmux command implementation has **moderate race condition risks** in concurrent scenarios, but is **safe for typical single-agent use**. The main risks are:

1. ✅ **Sequential operations protected** - Text + Enter are sent in sequence
2. ⚠️ **No pane-level locking** - Concurrent sends to same pane can interleave
3. ⚠️ **State check → action gap** - Pane state can change between check and send
4. ✅ **Read-only commands safe** - List commands are idempotent

---

## Race Condition Scenarios

### 1. Concurrent `send-prompt` to Same Pane ⚠️ **MODERATE RISK**

**Scenario:**
```bash
# Process A and B both send to same pane simultaneously
Process A: nabi tmux send-prompt myses:1 "command_a"
Process B: nabi tmux send-prompt myses:1 "command_b"
```

**What happens:**
- Both processes check pane state (lines 686-701)
- Both send Ctrl+C recovery (if needed)
- Both send text (lines 734-749)
- Both send Enter (lines 755-770)
- **Result**: Commands interleave → `command_a` and `command_b` mixed

**Current Protection:**
- Sequential operations with delays
- No explicit locking mechanism
- tmux may serialize operations internally (not guaranteed)

**Risk Level:** MODERATE - Only affects concurrent sends to same pane

**Mitigation Options:**
1. Add file-based locking per pane (e.g., `/tmp/nabi-tmux-lock-{session}:{window}.{pane}`)
2. Use tmux's `lock-session` command (locks entire session)
3. Document that concurrent sends to same pane are not supported
4. Add `--lock` flag for explicit locking

---

### 2. Pane State Changes Between Check and Send ⚠️ **LOW RISK**

**Scenario:**
```bash
# User types in pane while nabi is sending
1. nabi checks: pane has shell (zsh)
2. User switches to vim
3. nabi sends text → goes to vim instead of shell
```

**What happens:**
- Check foreground process (line 686)
- User changes pane state
- Send keys (line 734) → wrong target

**Current Protection:**
- Fast operation (check → send is ~20ms)
- Low probability in practice

**Risk Level:** LOW - Very short window, unlikely in practice

**Mitigation:**
- Re-check state just before sending (adds overhead)
- Accept risk (current approach)

---

### 3. Session/Window/Pane Deletion ⚠️ **LOW RISK**

**Scenario:**
```bash
# Session deleted between query and use
1. nabi tmux list windows myses → returns windows
2. User kills session
3. nabi tmux send-prompt myses:1 "cmd" → fails gracefully
```

**What happens:**
- List commands query state
- Session deleted
- Send command fails with error (handled)

**Current Protection:**
- Error handling in place
- Fails gracefully with clear error message

**Risk Level:** LOW - Handled gracefully

---

### 4. Current Session Detection ⚠️ **LOW RISK**

**Scenario:**
```bash
# Session changes between detection and use
1. nabi detects: current session = "session-a"
2. User switches to "session-b"
3. nabi uses "session-a" → may fail or target wrong session
```

**What happens:**
- `get_current_tmux_session()` called (line 240)
- User switches sessions
- Command uses stale session name

**Current Protection:**
- Fast operation
- Error handling if session doesn't exist

**Risk Level:** LOW - Short window, error handling in place

---

## Recommendations

### For Single-Agent Use (Current) ✅
**Status:** SAFE
- No concurrent sends expected
- Race conditions unlikely
- Current implementation is sufficient

### For Multi-Agent Coordination ⚠️
**Status:** NEEDS IMPROVEMENT
- Add pane-level locking for `send-prompt`
- Consider using tmux's `lock-session` for critical operations
- Document concurrent access limitations

### Implementation Options

**Option 1: File-based Locking (Recommended)**
```rust
// Lock file: /tmp/nabi-tmux-lock-{session}:{window}.{pane}
// Use flock() or file creation as mutex
// Release after send completes
```

**Option 2: Tmux Session Locking**
```rust
// Use: tmux lock-session -t {session}
// Send commands
// Use: tmux unlock-session -t {session}
// Note: Locks entire session, not just pane
```

**Option 3: Accept Risk + Document**
- Document that concurrent sends to same pane are not supported
- Recommend coordination at higher level (e.g., agent-level queuing)

---

## Current Safety Guarantees

✅ **Sequential Delivery**: Text and Enter are guaranteed sequential within a single `send-prompt` call
✅ **Error Handling**: All operations check for success and fail gracefully
✅ **Read-Only Safety**: List commands are idempotent and safe for concurrent access
⚠️ **Concurrent Writes**: No protection against concurrent `send-prompt` to same pane

---

## Conclusion

The implementation is **safe for typical use cases** where:
- Single agent sends commands
- Commands are sent sequentially
- No concurrent access to same pane

For **multi-agent scenarios** with concurrent pane access, consider adding pane-level locking or coordination at a higher level.
