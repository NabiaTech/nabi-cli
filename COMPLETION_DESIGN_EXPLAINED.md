# Completion Design Explained - Why It's Breaking

**Date**: 2025-11-18
**Purpose**: Explain the actual design vs what we're trying to do, and why it keeps breaking

---

## What Clap Actually Generates

### The Static Completion File (from `cargo build` → `nabi completions zsh`)

Clap generates a zsh completion file with this structure:

```zsh
#compdef nabi

_nabi() {
    local context state line
    local -a words
    words=("${(@)words}")
    
    _arguments "${_arguments_options[@]}" : \
        ":: :_nabi_commands" \
        "*::: :->nabi"
    
    case $state in
    (nabi)
        case $line[1] in
        (tmux)
            _arguments "${_arguments_options[@]}" : \
                ":: :_nabi__tmux_commands" \
                "*::: :->tmux"
            ;;
        (port)
            _arguments "${_arguments_options[@]}" : \
                ":: :_nabi__port_commands" \
                "*::: :->port"
            ;;
        esac
        ;;
    (tmux)
        case $line[1] in
        (send-prompt)
            _arguments "${_arguments_options[@]}" : \
                ':pane -- Tmux pane target:_default' \
                ':message -- Message to send:_default' \
                && ret=0
            ;;
        esac
        ;;
    (port)
        case $line[1] in
        (shift)
            _arguments "${_arguments_options[@]}" : \
                ':service -- Service name:_default' \
                ':old_port -- Current port:_default' \
                ':new_port -- New port:_default' \
                && ret=0
            ;;
        esac
        ;;
    esac
}
```

**Key Point**: Clap uses `:_default` for ALL arguments that don't have explicit completion hints.

**What `:_default` does**: In zsh, `_default` is a completion function that:
- Completes files/directories by default
- Falls back to other completion types
- Is the "catch-all" completion

---

## What We're Trying To Do

### The Problem

1. **`nabi tmux send-prompt <TAB>`** → Clap generates `:_default` → Shows **files** (wrong!)
2. **`nabi port shift <TAB>`** → Clap generates `:_default` → Shows **files** (wrong!)
3. **`nabi events ack <TAB>`** → Clap generates `:_default` → Shows **files** (wrong!)

**We want**:
- `nabi tmux send-prompt <TAB>` → Show **tmux sessions/panes** (dynamic!)
- `nabi port shift <TAB>` → Show **service names from port registry** (dynamic!)
- `nabi events ack <TAB>` → Show **event IDs from storage** (dynamic!)

### Why Clap Can't Do This

**Clap limitations**:
- Clap generates completions at **build time** (static)
- Clap doesn't know about runtime state (tmux sessions, port registry, events)
- Clap can't query external systems during completion
- Clap only supports `:_default` for unknown argument types

**Clap's `ValueHint` enum** (from Rust code):
```rust
pub enum ValueHint {
    Unknown,
    Other,
    Path,           // File/directory
    CommandName,     // Command name
    Username,        // Username
    Hostname,        // Hostname
    Url,             // URL
    EmailAddress,    // Email
    // ... but NO: TmuxPane, ServiceName, EventId
}
```

We use `ValueHint::Other` for `pane` argument (line 118 in `tmux.rs`), which tells clap "this isn't a file", but clap still generates `:_default` because it doesn't know what else to do.

---

## The Current (Broken) Approach

### What We're Actually Doing

**File**: `contrib/nabi-completions-dynamic.zsh`

```zsh
# Hook into the main _nabi completion function
if (( $+functions[_nabi] )); then
    # Save original _nabi function
    functions[_nabi_original]=$functions[_nabi]
    
    # Override _nabi with our wrapper
    _nabi() {
        # Check context: are we completing "nabi tmux send-prompt <pane>"?
        if [[ "${words[1]}" == "nabi" ]] && \
           [[ "${words[2]}" == "tmux" ]] && \
           [[ "${words[3]}" == "send-prompt" ]] && \
           [[ $CURRENT -eq 4 ]]; then
            # Yes! Call our dynamic completion
            _nabi_tmux_send_prompt_pane
            return 0
        fi
        
        # Check context: are we completing "nabi port shift <service>"?
        if [[ "${words[1]}" == "nabi" ]] && \
           [[ "${words[2]}" == "port" ]] && \
           [[ "${words[3]}" == "shift" ]] && \
           [[ $CURRENT -eq 4 ]]; then
            # Yes! Call our dynamic completion
            _nabi_port_shift_service
            return 0
        fi
        
        # For everything else, call the original clap-generated completion
        _nabi_original "$@"
    }
fi
```

### Why This Is Wrong

**Problem 1: We're overriding the entire dispatcher**
- `_nabi` is the main completion function that routes to all subcommands
- We're wrapping the entire thing instead of just the specific arguments
- This is like overriding `main()` instead of fixing a specific function

**Problem 2: Autoload conflict**
- zsh autoloads `_nabi` from the file (makes it a stub)
- When we try to save `functions[_nabi_original]=$functions[_nabi]`, we're saving a stub
- The stub says "autoload `_nabi_original` from file", but file only has `_nabi`
- When `_nabi_original` is called, zsh tries to autoload it → FAILS

**Problem 3: Timing issues**
- When is `_nabi` autoloaded? When file is sourced? When compinit runs? When completion triggers?
- We can't reliably detect if it's a stub or real function
- Different zsh versions behave differently

---

## What We SHOULD Be Doing

### Option 1: Override `_default` Instead (Proper Way)

**Instead of overriding `_nabi`, override `_default`**:

```zsh
# Save original _default
if (( ! $+functions[_default_original] )); then
    functions[_default_original]=$functions[_default]
fi

# Override _default to check context
_default() {
    # Check if we're completing "nabi tmux send-prompt <pane>"
    if [[ "$curcontext" == *"nabi-tmux-send-prompt"* ]] && \
       [[ $CURRENT -eq 4 ]]; then
        _nabi_tmux_send_prompt_pane
        return 0
    fi
    
    # Check if we're completing "nabi port shift <service>"
    if [[ "$curcontext" == *"nabi-port-shift"* ]] && \
       [[ $CURRENT -eq 4 ]]; then
        _nabi_port_shift_service
        return 0
    fi
    
    # For everything else, use original _default
    _default_original "$@"
}
```

**Why This Is Better**:
- `_default` is called by `_arguments` when it sees `:_default`
- We're only overriding the completion function, not the dispatcher
- `_default` is always a real function (not autoloaded)
- No timing issues

**Why We're Not Doing This**:
- We tried this approach, but `curcontext` detection was unreliable
- Context strings from clap are complex and hard to match
- We couldn't reliably detect which argument we're completing

### Option 2: Override Specific `_arguments` Completions (Best Way)

**Instead of overriding `_nabi` or `_default`, override the specific completion functions**:

```zsh
# After clap generates completions, override specific argument completions
# This happens AFTER the static file is loaded

# Override the pane argument completion for send-prompt
_nabi__tmux__send_prompt_pane() {
    _nabi_tmux_send_prompt_pane
}

# Override the service argument completion for port shift
_nabi__port__shift_service() {
    _nabi_port_shift_service
}
```

**Then modify the generated `_arguments` call**:
```zsh
# Instead of:
':pane -- Tmux pane target:_default'

# We want:
':pane -- Tmux pane target:_nabi__tmux__send_prompt_pane'
```

**Why This Is Best**:
- Each argument has its own completion function
- No context detection needed
- No overriding main dispatcher
- Clean separation of concerns

**Why We're Not Doing This**:
- Clap generates the `_arguments` call - we can't modify it
- We'd need to post-process the generated file (sed/awk)
- More complex, but more reliable

### Option 3: Use Clap's Completion Hints Properly (Ideal, But Not Possible)

**What we want**:
```rust
#[arg(value_name = "PANE", completion = "tmux_pane")]
pane: String,
```

**Why we can't**:
- Clap doesn't support custom completion functions
- Clap only supports built-in `ValueHint` enum values
- No way to say "use this zsh function for completion"

---

## The Real Solution

### What You're Asking For

> "Why don't we just have a single canonical truth that is generated on cargo build or cargo build --release. If that does not automatically copy or replace completions that's what the justfile is for to do the second stage of deployment."

**You're right!** Here's what SHOULD happen:

### Proper Design

1. **Cargo/clap generates completions** → `.build/_nabi_static`
   - This is the canonical truth
   - Contains ALL static completions
   - No modifications needed

2. **Justfile post-processes** → Replaces `:_default` with specific functions
   ```bash
   # In justfile:
   # Replace ':pane -- ...:_default' with ':pane -- ...:_nabi__tmux__send_prompt_pane'
   sed -i 's/:pane --.*:_default/:pane -- Tmux pane target:_nabi__tmux__send_prompt_pane/' .build/_nabi_static
   sed -i 's/:service --.*:_default/:service -- Service name:_nabi__port__shift_service/' .build/_nabi_static
   ```

3. **Append dynamic functions** → Add our completion functions
   ```bash
   echo '' >> .build/_nabi_static
   echo '# Dynamic completion functions' >> .build/_nabi_static
   cat contrib/nabi-completions-dynamic.zsh >> .build/_nabi_static
   ```

4. **No overriding `_nabi`** → Just define the specific completion functions
   ```zsh
   # In dynamic file - NO overriding _nabi!
   # Just define the completion functions:
   
   _nabi__tmux__send_prompt_pane() {
       _nabi_tmux_send_prompt_pane
   }
   
   _nabi__port__shift_service() {
       _nabi_port_shift_service
   }
   ```

**Benefits**:
- ✅ Single canonical truth (clap-generated)
- ✅ Post-processing in justfile (deployment stage)
- ✅ No overriding main dispatcher
- ✅ No autoload issues
- ✅ No fragile workarounds
- ✅ Stable and reliable

---

## Why We're Not Doing This (Current State)

**The current approach exists because**:
1. We didn't realize we could post-process the generated file
2. We thought we had to override `_nabi` to intercept completions
3. We didn't understand zsh completion system well enough
4. We kept adding workarounds instead of fixing the design

**The workarounds**:
- Override `_nabi` → Fragile, breaks with autoload
- Save `_nabi_original` → Breaks because it's a stub
- Detect autoload → Complex, unreliable
- Context detection → Unreliable, breaks easily

**Result**: Breaks on every commit because the workaround is fragile.

---

## The Fix

### Step 1: Stop Overriding `_nabi`

Remove all the `_nabi` override code from `contrib/nabi-completions-dynamic.zsh`.

### Step 2: Post-Process Generated Completions

In `justfile`, after generating static completions:
```bash
# Replace _default with specific completion functions
sed -i 's/:pane -- Tmux pane target:_default/:pane -- Tmux pane target:_nabi__tmux__send_prompt_pane/' .build/_nabi_static
sed -i 's/:service -- Service name to migrate:_default/:service -- Service name to migrate:_nabi__port__shift_service/' .build/_nabi_static
```

### Step 3: Define Specific Completion Functions

In `contrib/nabi-completions-dynamic.zsh`:
```zsh
# NO _nabi override! Just define the completion functions:

_nabi__tmux__send_prompt_pane() {
    _nabi_tmux_send_prompt_pane
}

_nabi__port__shift_service() {
    _nabi_port_shift_service
}

_nabi__events__ack_event_id() {
    _nabi_events_ack_event_id
}
```

### Step 4: Remove All Workarounds

- No `_nabi_original`
- No autoload detection
- No context detection in `_nabi`
- Just clean, simple completion functions

---

## Summary

**What you want**: ✅ Single canonical truth from cargo/clap, post-processed in justfile

**What we're doing**: ❌ Overriding main dispatcher with fragile workarounds

**Why it breaks**: Autoload makes function stubs, we can't save/restore them reliably

**The fix**: Post-process generated file to replace `:_default` with specific functions, define those functions in dynamic file, no overriding needed

**Result**: Stable, reliable completions that don't break on every commit.
