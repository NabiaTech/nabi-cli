# Analysis: Adding Session Completions to `nabi tmux send-prompt`

## Current State

### Problem
When typing `nabi tmux send-prompt <TAB>`, the shell is completing **files** instead of **available tmux sessions** (in `session:window.pane` format).

### Existing Infrastructure

1. **Dynamic Completion Script**: `contrib/nabi-completions-dynamic.zsh`
   - Already attempts to provide completions for `send-prompt`
   - Hooks into zsh's completion system via `_default` override
   - Has helper functions to query tmux sessions/windows/panes

2. **CLI Commands Available**:
   - `nabi tmux list sessions --format name` → Returns session names (one per line)
   - `nabi tmux list windows <session> --format id` → Returns window IDs
   - `nabi tmux list panes <session>:<window> --format full` → Returns `session:window.pane` format

3. **Clap Configuration**:
   - Uses `clap_complete` for static shell completion generation
   - The `pane` argument in `SendPrompt` is currently a plain `String` with no hints

## Root Causes

### Issue 1: Dynamic Script Not Using Correct Format Flags
The dynamic completion script calls:
- `nabi tmux list sessions` (without `--format name`) → Gets JSON instead of plain names
- `nabi tmux list windows "$session"` (without `--format id`) → Gets JSON instead of IDs
- `nabi tmux list panes "$session" "$window"` (without `--format full`) → Gets JSON instead of pane targets

**Location**: `contrib/nabi-completions-dynamic.zsh` lines 15, 26, 38

### Issue 2: Clap Defaults to File Completion
Since the `pane` argument is a plain `String` with no `value_hint`, clap's generated completions default to file completion.

**Location**: `src/commands/tmux.rs` line 108-109

### Issue 3: Dynamic Script May Not Be Loaded
The script needs to be sourced in `.zshrc` after the base `_nabi` completion, which may not be happening.

## Solutions

### Solution 1: Fix Dynamic Completion Script (Recommended - Quick Fix)

**Changes needed in `contrib/nabi-completions-dynamic.zsh`:**

1. **Fix session listing** (line 15):
```zsh
# OLD:
sessions=($(nabi tmux list sessions 2>/dev/null | tr '\n' ' '))

# NEW:
sessions=($(nabi tmux list sessions --format name 2>/dev/null))
```

2. **Fix window listing** (line 26):
```zsh
# OLD:
windows=($(nabi tmux list windows "$session" 2>/dev/null | tr '\n' ' '))

# NEW:
windows=($(nabi tmux list windows "$session" --format id 2>/dev/null))
```

3. **Fix pane listing** (line 38):
```zsh
# OLD:
panes=($(nabi tmux list panes "$session" "$window" 2>/dev/null | tr '\n' ' '))

# NEW:
# Use full format to get session:window.pane directly
panes=($(nabi tmux list panes "${session}:${window}" --format full 2>/dev/null))
```

4. **Fix pane completion logic** (lines 42-78):
The current logic tries to build pane targets manually, but we can use `--format full` to get them directly:

```zsh
_nabi_tmux_send_prompt_pane() {
    local -a targets

    # Get all sessions
    local -a sessions
    sessions=($(nabi tmux list sessions --format name 2>/dev/null))

    if (( ${#sessions} == 0 )); then
        _message "No tmux sessions found"
        return 1
    fi

    # For each session, get all windows
    for session in $sessions; do
        local -a windows
        windows=($(nabi tmux list windows "$session" --format id 2>/dev/null))

        # Add session:window format (implicit pane 1)
        for window in $windows; do
            targets+=("${session}:${window}")
        done

        # Get all panes in full format (session:window.pane)
        for window in $windows; do
            local -a panes
            panes=($(nabi tmux list panes "${session}:${window}" --format full 2>/dev/null))
            targets+=($panes)
        done
    done

    # Remove duplicates and sort
    targets=(${(u)targets})

    _describe 'pane target' targets
}
```

### Solution 2: Add ValueHint to Prevent File Completion

**Changes needed in `src/commands/tmux.rs`:**

Add `value_hint` to the pane argument to prevent clap from defaulting to file completion:

```rust
#[arg(value_name = "PANE", value_hint = clap::ValueHint::Other)]
pane: String,
```

This tells clap that this isn't a file path, so it won't generate file completions.

### Solution 3: Improve Completion Detection Logic

The current detection logic in `_default()` function checks for `send-prompt` in context and word count. This may not be reliable. Better approach:

```zsh
_default() {
    local context="$curcontext"
    local -a words
    words=(${(z)BUFFER})

    # More precise detection: nabi tmux send-prompt <pane>
    if [[ "$context" == *"nabi"* ]] && [[ "$context" == *"tmux"* ]]; then
        # Check if we're completing the pane argument for send-prompt
        # Pattern: nabi tmux send-prompt <TAB>
        if [[ "${words[1]}" == "nabi" ]] && \
           [[ "${words[2]}" == "tmux" ]] && \
           [[ "${words[3]}" == "send-prompt" ]] && \
           [[ ${#words} -eq 4 ]]; then
            _nabi_tmux_send_prompt_pane
            return
        fi

        # ... other cases ...
    fi

    _nabi_original_default "$@"
}
```

## Implementation Plan

### Phase 1: Quick Fix (Immediate)
1. ✅ Fix format flags in dynamic completion script
2. ✅ Add `value_hint` to pane argument
3. ✅ Improve detection logic

### Phase 2: Testing
1. Test with multiple sessions/windows/panes
2. Test with no tmux sessions running
3. Test with partial input (e.g., `nabi tmux send-prompt agent-<TAB>`)

### Phase 3: Documentation
1. Update `contrib/README-completions.md` with troubleshooting
2. Add installation verification steps

## Alternative Approaches Considered

### Option A: Custom Clap Value Parser
- **Pros**: Integrated with clap, works across all shells
- **Cons**: Clap completions are static (generated at build time), can't query tmux at runtime
- **Verdict**: Not suitable for dynamic tmux session completion

### Option B: Shell-Specific Completion Scripts
- **Pros**: Can provide dynamic completions per shell
- **Cons**: Need separate scripts for bash/zsh/fish
- **Verdict**: Current zsh-only approach is fine for now, can extend later

### Option C: Completion Server/Subcommand
- **Pros**: Could provide completions via `nabi _completions tmux send-prompt pane`
- **Cons**: More complex, requires shell integration anyway
- **Verdict**: Overkill for this use case

## Files to Modify

1. `core/nabi-cli/contrib/nabi-completions-dynamic.zsh` - Fix format flags and logic
2. `core/nabi-cli/src/commands/tmux.rs` - Add `value_hint` to pane argument
3. `core/nabi-cli/contrib/README-completions.md` - Update documentation

## Testing Checklist

- [ ] `nabi tmux send-prompt <TAB>` shows session:window.pane completions
- [ ] `nabi tmux send-prompt agent-<TAB>` filters to matching sessions
- [ ] Works with multiple sessions
- [ ] Works with sessions that have multiple windows/panes
- [ ] Gracefully handles no tmux sessions (shows message)
- [ ] Doesn't show file completions anymore
