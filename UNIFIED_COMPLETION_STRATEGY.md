# Unified Completion Strategy - The Right Solution

**Date**: 2025-11-18
**Status**: ✅ **COMPREHENSIVE SOLUTION**
**Scope**: ALL dynamic completions (tmux, port, events, exec, tool exec)

---

## The Problem: Two Inconsistent Strategies

### Current State Analysis

**Strategy A: Override `_nabi` Main Dispatcher** (Lines 44-134 in dynamic file)
- Used for: `port shift`, `tmux send-prompt`, `events ack`, `exec`, `tool exec`
- Problem: Autoload stub issues, fragile, breaks with cache
- Why it breaks: Tries to copy `functions[_nabi]` which might be a stub

**Strategy B: Override Specific Functions** (Lines 401-410 in dynamic file)
- Used for: `_nabi__exec_commands`, `_nabi__tool__exec_commands`
- Works better: No autoload stub issues
- Problem: Inconsistent with Strategy A

**Result**: Mixed approach causes confusion and breakage.

---

## The Root Cause

### What Clap Generates

All dynamic arguments use either:
1. **No completion function**: `'::pane -- ...:'` (just `:`)
2. **Default completion**: `':service -- ...:_default'` (uses `:_default`)

Examples from generated file:
- `nabi tmux send-prompt <PANE>` → `'::pane -- Tmux pane target:...'` (no function)
- `nabi port shift <SERVICE>` → `':service -- Service name:_default'`
- `nabi exec <TOOL>` → `':tool -- Tool ID:_default'`
- `nabi tool exec <TOOL>` → `':tool -- Tool ID:_default'`
- `nabi events ack <EVENT_ID>` → `':event_id -- Event ID:_default'`

### Why Current Approach Fails

1. **Override `_nabi`** → Intercepts `_default` calls
2. **Copy `functions[_nabi]`** → Might be autoload stub (`"builtin autoload -XU"`)
3. **Cache mismatch** → File updated but cache has old version
4. **Timing dependency** → Works sometimes, breaks other times

---

## The Unified Solution

### Principle: Single Canonical Truth + Post-Processing

**Step 1: Clap Generates Completions** (Canonical Truth)
```bash
cargo build --release
nabi completions zsh > .build/_nabi_static
```

**Step 2: Post-Process Generated File** (In Justfile)
Replace `:` or `:_default` with specific completion functions using context-aware patterns:

```bash
# Replace pane argument for send-prompt
sed -i '' 's/::pane -- Tmux pane target.*:/&_nabi__tmux__send_prompt_pane/' .build/_nabi_static

# Replace service argument for port shift
sed -i '' 's/:service -- Service name to migrate:_default/:service -- Service name to migrate:_nabi__port__shift_service/' .build/_nabi_static

# Replace tool argument for nabi exec (top-level)
# Need context: match (exec) case followed by :tool
awk '/^\(exec\)$/,/^;;$/ {
    if (/:tool -- Tool ID.*:_default/) {
        sub(/:_default/, ":_nabi__exec_tool")
    }
}1' .build/_nabi_static > .build/_nabi_static.tmp && mv .build/_nabi_static.tmp .build/_nabi_static

# Replace tool argument for nabi tool exec (nested)
awk '/^\(tool\)$/,/^;;$/ {
    if (/\(exec\)/ && /:tool -- Tool ID.*:_default/) {
        sub(/:_default/, ":_nabi__tool__exec_tool")
    }
}1' .build/_nabi_static > .build/_nabi_static.tmp && mv .build/_nabi_static.tmp .build/_nabi_static

# Replace event_id argument for events ack
sed -i '' 's/:event_id -- Event ID.*:_default/:event_id -- Event ID:_nabi__events__ack_event_id/' .build/_nabi_static
```

**Step 3: Define Completion Functions** (In Dynamic File)
```zsh
# NO _nabi override! Just define the functions:

_nabi__tmux__send_prompt_pane() {
    _nabi_tmux_send_prompt_pane
}

_nabi__port__shift_service() {
    _nabi_port_shift_service
}

_nabi__exec_tool() {
    _nabi_tool_exec_tool_completion
}

_nabi__tool__exec_tool() {
    _nabi_tool_exec_tool_completion
}

_nabi__events__ack_event_id() {
    _nabi_events_ack_event_id
}
```

**Step 4: Append Dynamic File** (As Before)
```bash
echo '' >> .build/_nabi_static
echo '# Dynamic completion functions' >> .build/_nabi_static
tail -n +2 contrib/nabi-completions-dynamic.zsh >> .build/_nabi_static
```

---

## Complete Mapping

| Command | Argument | Generated Pattern | Replacement Function |
|---------|----------|-------------------|---------------------|
| `nabi tmux send-prompt <PANE>` | `pane` | `'::pane -- ...:'` | `_nabi__tmux__send_prompt_pane` |
| `nabi port shift <SERVICE>` | `service` | `':service -- ...:_default'` | `_nabi__port__shift_service` |
| `nabi exec <TOOL>` | `tool` | `':tool -- ...:_default'` | `_nabi__exec_tool` |
| `nabi tool exec <TOOL>` | `tool` | `':tool -- ...:_default'` | `_nabi__tool__exec_tool` |
| `nabi events ack <EVENT_ID>` | `event_id` | `':event_id -- ...:_default'` | `_nabi__events__ack_event_id` |

---

## Benefits of This Approach

### ✅ No Autoload Stub Issues
- We're not copying functions
- We're defining functions directly
- No dependency on `functions[_nabi]` state

### ✅ Works With Cache or Without Cache
- Functions are defined in the file
- Cache loads them normally
- No timing dependencies

### ✅ Single Canonical Truth
- Clap generates the base file
- Justfile post-processes it
- Dynamic file defines functions
- No overriding needed

### ✅ Stable and Predictable
- No fragile workarounds
- No context detection in runtime
- No autoload detection logic
- Just simple function definitions

### ✅ Easy to Extend
- Add new dynamic completion?
  1. Add sed/awk replacement in justfile
  2. Define function in dynamic file
  3. Done!

---

## Implementation Plan

### Phase 1: Update Justfile
1. Add post-processing step after generating static completions
2. Use sed/awk to replace `:` or `:_default` with specific functions
3. Test that replacements work correctly

### Phase 2: Update Dynamic File
1. Remove ALL `_nabi` override code (lines 44-134)
2. Remove ALL `_nabi_original` handling
3. Remove `_nabi__exec_commands` override (lines 401-404)
4. Remove `_nabi__tool__exec_commands` override (lines 407-410)
5. Define ONLY the specific completion functions

### Phase 3: Test
1. Fresh cache: `rm ~/.cache/zsh/.zcompdump-*` then test
2. Stale cache: Wait 20+ hours, test
3. After `just install`: Test immediately
4. All dynamic completions: `nabi exec <TAB>`, `nabi tmux send-prompt <TAB>`, etc.

### Phase 4: Invalidate Cache on Install (Optional)
Add to justfile:
```bash
# Invalidate completion cache so next shell rebuilds it
rm -f "${HOME}/.cache/zsh/.zcompdump-${ZSH_VERSION}"
```

This ensures cache is always fresh after `just install`, but isn't strictly necessary with the new approach.

---

## Why This Is The Right Solution

### What You Asked For
> "Why don't we just have a single canonical truth that is generated on cargo build or cargo build --release. If that does not automatically copy or replace completions that's what the justfile is for to do the second stage of deployment."

**This is exactly that!**
- ✅ Single canonical truth: Clap generates it
- ✅ Post-processing in justfile: Replaces `:` or `:_default` with specific functions
- ✅ No hacky workarounds: Just function definitions
- ✅ Stable: Works with cache, without cache, autoloaded or not

### Why It Works
- **No overriding**: We're not fighting zsh's completion system
- **No copying**: We're not copying function stubs
- **No timing**: Functions are defined, not dynamically created
- **No cache issues**: Cache loads functions normally

---

## Summary

**Current State**: Two inconsistent strategies, fragile overrides, autoload stub issues, cache mismatches

**Target State**: Single canonical truth, post-processed in justfile, simple function definitions, stable and predictable

**Key Change**: Stop overriding `_nabi`. Instead, post-process the generated file to use specific completion functions, then define those functions directly.

**Result**: Completions that work reliably, don't break on every commit, and are easy to maintain and extend.
