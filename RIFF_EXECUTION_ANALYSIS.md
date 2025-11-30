# Riff Execution Path Analysis

**Date**: 2025-11-29  
**Purpose**: Understand why `riff` is unreliable and map all execution paths

## Executive Summary

`riff` has **three execution paths** that all converge to the same Python code, but with different routing overhead:

1. **Direct `riff` command** → Wrapper script → `nabi tool exec riff-cli` → Tool manifest → Python execution
2. **`nabi riff`** → Rust handler → `route_to_python_cli` → `nabi-python` → Generated router → Python execution  
3. **`nabi tool exec riff-cli`** → Tool manifest → Python execution

All paths execute: `/Users/tryk/.nabi/venvs/riff-cli/bin/python -m riff.cli`

## Execution Path Details

### Path 1: Direct `riff` Command

```
riff graph <session_id>
  ↓
/Users/tryk/.local/bin/riff (wrapper script)
  ↓
exec nabi tool exec riff-cli -- "$@"
  ↓
handle_tool_exec("riff-cli", ["graph", ...])
  ↓
Load manifest: ~/.config/nabi/tools/riff-cli.toml
  ↓
Execute: python -m riff.cli graph <session_id>
  ↓
~/.nabi/venvs/riff-cli/bin/python -m riff.cli graph <session_id>
```

**Characteristics**:
- Shows: `→ Tool not promoted, executing from source`
- Executes from: `~/nabia/tools/riff-cli/src/riff/cli.py`
- Uses tool manifest system

### Path 2: `nabi riff` Command

```
nabi riff graph <session_id>
  ↓
handle_riff(["graph", ...]) in main.rs:5165
  ↓
route_to_python_cli(["riff", "graph", ...])
  ↓
~/.local/share/nabi/bin/nabi-python riff graph <session_id>
  ↓
nabi-python script sources: ~/.local/state/nabi/tools/layer2-router-generated.sh
  ↓
Generated router matches "riff" case
  ↓
exec ~/.nabi/venvs/riff-cli/bin/python -m riff.cli graph <session_id>
```

**Characteristics**:
- Shows: `🔍 Routing to riff-cli...`
- Sometimes shows: `⚠️ Router timeout, using fallback routing` (then retries)
- Uses Python CLI layer routing
- May have timeout mechanism (needs investigation)

### Path 3: `nabi tool exec riff-cli`

```
nabi tool exec riff-cli -- graph <session_id>
  ↓
handle_tool_exec("riff-cli", ["graph", ...])
  ↓
(Same as Path 1 from here)
```

## Performance Comparison

From timing tests:
- **Direct `riff`**: ~4.5 seconds (includes tool manifest loading)
- **`nabi riff`**: ~0.01 seconds (fast routing, but may timeout)
- **`nabi tool exec`**: ~4.5 seconds (same as direct)

## Core Issue: Empty Session Handling

**Problem**: All paths fail with the same error for empty sessions:

```python
File "/Users/tryk/nabia/tools/riff-cli/src/riff/graph/dag.py", line 55
    raise ValueError(f"Session {session_id} contains no messages")
```

**Root Cause**: `ConversationDAG.__init__()` raises `ValueError` when `loader.load_messages(session_id)` returns empty list.

**Location**: `/Users/tryk/nabia/tools/riff-cli/src/riff/graph/dag.py:54-55`

**Expected Behavior**: Should handle empty sessions gracefully with informative message instead of crashing.

## Architecture Issues

### 1. Unused Handler Stub
- `nabi-cli/src/handlers/riff.rs` exists but is never imported/used
- Actual handler is in `main.rs:5165` (`handle_riff()`)
- Handler module declared in `handlers/mod.rs` but not referenced

### 2. Router Timeout Mystery
- `nabi riff` sometimes shows: `⚠️ Router timeout, using fallback routing`
- Message source not found in:
  - `nabi-cli/src/main.rs`
  - `nabi-cli/src/routing.rs`
  - `/Users/tryk/.local/share/nabi/bin/nabi-python`
  - Generated router script
- **Needs investigation**: May be in Python CLI layer or timeout wrapper

### 3. Inconsistent Routing Messages
- Direct `riff`: Shows "Tool not promoted, executing from source"
- `nabi riff`: Shows "Routing to riff-cli..." (and sometimes timeout warning)
- Both execute same code, but different messaging

## Tool Manifest Configuration

**Location**: `~/.config/nabi/tools/riff-cli.toml`

```toml
[runtime]
language = "python"
version = "3.13+"
entry_point = "riff"
execution = "python -m riff.cli"
wrapper = "nabi"

[venv]
location = "~/.nabi/venvs/riff-cli"
```

**Execution**: `python -m riff.cli` (resolved to venv Python)

## Recommendations

### Immediate Fixes

1. **Fix Empty Session Handling**
   - Modify `riff/graph/dag.py:54-55` to handle empty sessions gracefully
   - Return informative message instead of raising ValueError
   - Example: "Session exists but contains no messages. This may be a session that was created but never used."

2. **Investigate Router Timeout**
   - Find source of "Router timeout" message
   - Determine if timeout is intentional or bug
   - Fix or document timeout behavior

### Architecture Improvements

1. **Consolidate Handler**
   - Move `handle_riff()` from `main.rs` to `handlers/riff.rs`
   - Update imports to use handler module
   - Remove duplicate code

2. **Unify Routing Messages**
   - Standardize messaging across all execution paths
   - Remove "Tool not promoted" message if promotion system isn't used
   - Add consistent routing indicators

3. **Performance Optimization**
   - `nabi riff` is fastest (~0.01s) but may timeout
   - Direct `riff` is slow (~4.5s) due to manifest loading
   - Consider caching manifest or optimizing tool exec path

## Testing Results

### Empty Session Test
```bash
$ riff graph 7b1f653c-0bca-4069-bf53-f2814dfe5daf
→ Tool not promoted, executing from source
Error: Session 7b1f653c-0bca-4069-bf53-f2814dfe5daf contains no messages
ValueError: Session 7b1f653c-0bca-4069-bf53-f2814dfe5daf contains no messages
```

### Session With Messages Test
```bash
$ nabi riff graph d28a5eff-a9a8-4364-887b-1d830c040037
🔍 Routing to riff-cli...
⚠️ Router timeout, using fallback routing
Error visualizing conversation: nocbreak() returned ERR
```

(Note: Different error - curses/TUI issue, not routing issue)

## Files Involved

- **Rust Handler**: `nabi-cli/src/main.rs:5165` (`handle_riff()`)
- **Stub Handler**: `nabi-cli/src/handlers/riff.rs` (unused)
- **Python Entry**: `/Users/tryk/nabia/tools/riff-cli/src/riff/cli.py`
- **DAG Logic**: `/Users/tryk/nabia/tools/riff-cli/src/riff/graph/dag.py:54-55`
- **Wrapper Script**: `/Users/tryk/.local/bin/riff`
- **Python Router**: `/Users/tryk/.local/share/nabi/bin/nabi-python`
- **Generated Router**: `~/.local/state/nabi/tools/layer2-router-generated.sh`
- **Tool Manifest**: `~/.config/nabi/tools/riff-cli.toml`

## Next Steps

1. ✅ Map all execution paths (DONE)
2. ✅ Fix empty session handling in Python code (DONE - 2025-11-29)
3. ⏳ Investigate router timeout mechanism
4. ⏳ Consolidate handler architecture
5. ✅ Test fixes with real sessions (DONE - graceful error confirmed)

## Fix Applied (2025-11-29)

### Empty Session Handling Fix

**Problem**: `riff graph` crashed with `ValueError` for empty sessions.

**Solution**: 
- Added early check in `cmd_graph()` before creating DAG
- Provides informative error message instead of crashing
- Exits gracefully with code 1
- Optimized to avoid loading messages twice

**Files Modified**:
- `/Users/tryk/nabia/tools/riff-cli/src/riff/cli.py:706-715` - Early empty session check
- `/Users/tryk/nabia/tools/riff-cli/src/riff/graph/dag.py:36-56` - Made messages parameter optional

**Result**: 
```bash
$ riff graph 7b1f653c-0bca-4069-bf53-f2814dfe5daf
⚠️  Session 7b1f653c-0bca-4069-bf53-f2814dfe5daf exists but contains no messages
This may be a session that was created but never used, or contains only metadata records.
Tip: Use 'nabi recover sessions' to find sessions with actual content.
```

Exit code: 1 (graceful error, no crash)

