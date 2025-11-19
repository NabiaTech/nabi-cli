# What Is An "Autoloaded Stub"? (Zsh Completion System Explained)

**Date**: 2025-11-18
**Purpose**: Explain why copying `functions[_nabi]` breaks completions

---

## Normal Functions vs Autoloaded Functions

### Normal Function (Loaded Immediately)

When you define a function normally, zsh loads it into memory immediately:

```zsh
# Normal function definition
my_function() {
    echo "Hello, world!"
}

# Check what's stored
echo "${functions[my_function]}"
# Output: my_function() {
#            echo "Hello, world!"
#         }
```

**What happens**: The entire function body is stored in memory.

### Autoloaded Function (Lazy Loading)

When zsh sees a function file (like `~/.zsh/completions/_nabi`), it doesn't load the function immediately. Instead, it creates a **stub** that says "when this function is called, load it from the file."

```zsh
# File: ~/.zsh/completions/_nabi
#compdef nabi

_nabi() {
    local context state line
    # ... 8000+ lines of completion code ...
}
```

**What zsh does**:
1. Sees the file `_nabi` exists
2. Creates a stub: `functions[_nabi] = "builtin autoload -XU"`
3. **Does NOT load the function body**
4. When `_nabi` is called, zsh:
   - Reads the file
   - Loads the function body
   - Executes it

---

## What The Stub Actually Contains

### Before Function Is Called

```zsh
# Check what's in functions[_nabi] before calling it
echo "${functions[_nabi]}"
# Output: builtin autoload -XU
```

**This is just a marker!** It's not the actual function code. It's zsh's way of saying:
- "This function exists"
- "But I haven't loaded it yet"
- "When you call it, I'll load it from the file"

### After Function Is Called

```zsh
# Call the function (triggers autoload)
_nabi 2>&1 | head -1

# Now check what's in functions[_nabi]
echo "${functions[_nabi]:0:200}"
# Output: _nabi() {
#             local context state line
#             local -a words
#             words=("${(@)words}")
#             ...
#         }
```

**Now it's the real function!** After the first call, zsh loads the actual function body into memory.

---

## Why This Breaks Our Code

### What We're Trying To Do

```zsh
# In contrib/nabi-completions-dynamic.zsh
if (( $+functions[_nabi] )); then
    # Save original _nabi function
    functions[_nabi_original]=$functions[_nabi]
    
    # Override _nabi
    _nabi() {
        # ... our custom logic ...
        _nabi_original "$@"  # Call the original
    }
fi
```

### What Actually Happens

**Step 1: File is sourced**
```zsh
# zsh sources ~/.zsh/completions/_nabi
# Creates stub: functions[_nabi] = "builtin autoload -XU"
```

**Step 2: Our dynamic code runs**
```zsh
# Check if _nabi exists
if (( $+functions[_nabi] )); then
    # ✅ TRUE - stub exists
    
    # Try to save original
    functions[_nabi_original]=$functions[_nabi]
    # ❌ PROBLEM: We just saved "builtin autoload -XU"!
    # Not the actual function code!
```

**Step 3: We override _nabi**
```zsh
# Override with our wrapper
_nabi() {
    # ... our logic ...
    _nabi_original "$@"  # Try to call original
}
```

**Step 4: User tries to complete**
```zsh
# User types: nabi tmux send-prompt <TAB>
# zsh calls: _nabi

# Our wrapper runs:
_nabi() {
    # ... our logic ...
    _nabi_original "$@"  # Call original
}

# zsh tries to call _nabi_original
# But functions[_nabi_original] = "builtin autoload -XU"
# zsh tries to autoload _nabi_original from file
# But file only has _nabi, not _nabi_original!
# ERROR: _nabi_original: function definition file not found
```

---

## Visual Timeline

```
Time 0: File exists
┌─────────────────────────────────────┐
│ ~/.zsh/completions/_nabi            │
│ (8000+ lines of code)               │
└─────────────────────────────────────┘
         ↓
Time 1: compinit runs
┌─────────────────────────────────────┐
│ functions[_nabi] =                  │
│   "builtin autoload -XU"            │
│ (STUB - not the real function!)    │
└─────────────────────────────────────┘
         ↓
Time 2: Dynamic file is sourced
┌─────────────────────────────────────┐
│ functions[_nabi_original] =        │
│   "builtin autoload -XU"           │
│ (COPIED THE STUB!)                 │
│                                     │
│ functions[_nabi] =                  │
│   "_nabi() { ... _nabi_original ... }"
│ (OUR WRAPPER)                      │
└─────────────────────────────────────┘
         ↓
Time 3: User presses TAB
┌─────────────────────────────────────┐
│ zsh calls: _nabi()                  │
│   → calls: _nabi_original()         │
│   → zsh sees: "builtin autoload -XU"│
│   → tries to load from file         │
│   → looks for _nabi_original         │
│   → NOT FOUND!                      │
│   → ERROR!                          │
└─────────────────────────────────────┘
```

---

## Why We Can't Just Copy The Stub

### The Stub Is Not The Function

The stub is just a **pointer** to the file, not the actual code:

```zsh
# Stub (what we get)
functions[_nabi] = "builtin autoload -XU"

# Real function (what we need)
functions[_nabi] = "_nabi() {
    local context state line
    # ... actual code ...
}"
```

### The File Only Has `_nabi`, Not `_nabi_original`

When zsh tries to autoload `_nabi_original`, it looks for:
- `~/.zsh/completions/_nabi_original` (doesn't exist!)
- Or `_nabi_original` function in the file (doesn't exist!)

The file only has `_nabi`, so autoload fails.

---

## Why Timing Matters

### The Problem: When Is The Function Loaded?

**Scenario 1: Function is called before our code runs**
```zsh
# User completes something else first
_nabi  # Triggers autoload, loads real function

# Now our code runs
functions[_nabi_original]=$functions[_nabi]
# ✅ This works! We copy the real function
```

**Scenario 2: Our code runs before function is called**
```zsh
# Our code runs first
functions[_nabi_original]=$functions[_nabi]
# ❌ This fails! We copy the stub

# Later, user tries to complete
_nabi_original  # Tries to autoload, fails
```

**We can't control which scenario happens!** It depends on:
- When `compinit` runs
- When the file is sourced
- What the user completes first
- zsh version differences

---

## The Real Solution

### Don't Copy The Function At All

Instead of copying `_nabi`, we should:

1. **Post-process the generated file** to replace `:` with specific completion functions
2. **Define those functions** in our dynamic file
3. **Never override `_nabi`**

### Example

**Generated file** (from clap):
```zsh
':pane -- Tmux pane target:'
```

**Post-processed** (in justfile):
```zsh
':pane -- Tmux pane target:_nabi__tmux__send_prompt_pane'
```

**Dynamic file** (defines the function):
```zsh
_nabi__tmux__send_prompt_pane() {
    _nabi_tmux_send_prompt_pane
}
```

**No overriding needed!** zsh calls `_nabi__tmux__send_prompt_pane` directly, which calls our dynamic function.

---

## Summary

**Autoloaded stub**:
- A marker that says "function exists in file, load it when called"
- Contains: `"builtin autoload -XU"`
- **Not** the actual function code

**Why copying it breaks**:
- We copy the stub, not the real function
- When we call the copied stub, zsh tries to autoload it
- But the file doesn't have a function with that name
- Error: "function definition file not found"

**The fix**:
- Don't copy functions at all
- Post-process generated file to use specific completion functions
- Define those functions directly
- No overriding, no copying, no stubs
