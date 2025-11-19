# Zsh Completion Cache Analysis - Why It's Breaking

**Date**: 2025-11-18
**Key Finding**: Your `.zshrc` uses `compinit -C` caching, which interacts poorly with our dynamic completion override

---

## Your .zshrc Configuration

### Lines 163-177: Completion Caching

```zsh
# Completion - Optimized for performance
# Only rebuild completion dump once per day
autoload -Uz compinit

# Use a faster cache location
ZSH_COMPDUMP="${ZSH_CACHE_DIR}/.zcompdump-${ZSH_VERSION}"

# Only run full compinit if cache is older than 20 hours
if [[ -n ${ZSH_COMPDUMP}(#qNmh-20) ]]; then
    # Cache is fresh, skip expensive checks
    compinit -C -d "${ZSH_COMPDUMP}"
else
    # Cache is stale, rebuild with audit
    compinit -d "${ZSH_COMPDUMP}"
fi
```

**What This Does**:
- **`compinit -C`**: "Use cache only, don't check for new files" (performance optimization)
- **`compinit`** (no `-C`): "Rebuild cache, check all files" (slower but accurate)

### Lines 179-183: Dynamic Completions Loaded AFTER compinit

```zsh
# Load nabi dynamic completions after compinit
# These add runtime-aware completions for tmux sessions, event IDs, port services
if [[ -f "$HOME/.zsh/completions/nabi-completions-dynamic.zsh" ]]; then
    source "$HOME/.zsh/completions/nabi-completions-dynamic.zsh"
fi
```

**Critical Issue**: Dynamic file is loaded AFTER compinit, but compinit might have cached `_nabi` already!

---

## The Cache Problem

### How Zsh Completion Caching Works

**Step 1: First Shell Startup (No Cache)**
```
1. compinit runs (no cache exists)
2. Scans ~/.zsh/completions/ for completion files
3. Finds _nabi file
4. Loads _nabi function into memory
5. Saves to cache: ~/.cache/zsh/.zcompdump-5.9
```

**Step 2: Subsequent Shell Starts (< 20 hours old)**
```
1. compinit -C runs (use cache only)
2. Loads _nabi from cache (FAST!)
3. Cache might have _nabi as:
   - Autoload stub: "builtin autoload -XU"
   - OR fully loaded function body
4. Dynamic file tries to override _nabi
5. ❌ PROBLEM: What's in cache might not match what's in file!
```

**Step 3: After `just install` (File Changed, Cache Stale)**
```
1. just install → regenerates ~/.zsh/completions/_nabi
2. File now has NEW version (with dynamic code appended)
3. But cache still has OLD version (< 20 hours old)
4. compinit -C loads OLD version from cache
5. Dynamic file tries to override OLD cached version
6. ❌ MISMATCH: File ≠ Cache
```

---

## Why This Causes The Autoload Stub Issue

### Scenario A: Cache Has Stub

```
Cache contains: functions[_nabi] = "builtin autoload -XU"
Dynamic file runs: functions[_nabi_original]=$functions[_nabi]
Result: _nabi_original = "builtin autoload -XU" (stub!)
When called: ERROR - tries to autoload _nabi_original from file
```

### Scenario B: Cache Has Function Body

```
Cache contains: functions[_nabi] = "_nabi() { ... }" (full function)
Dynamic file runs: functions[_nabi_original]=$functions[_nabi]
Result: _nabi_original = "_nabi() { ... }" (works!)
When called: Works fine
```

**The Problem**: We can't predict which scenario happens! It depends on:
- When cache was last rebuilt
- What was in cache when it was saved
- Whether compinit -C or compinit ran

---

## The Real Issue: Cache Invalidation

### What Should Happen

When `just install` regenerates completions:
1. ✅ File is updated: `~/.zsh/completions/_nabi`
2. ❌ Cache is NOT invalidated: `~/.cache/zsh/.zcompdump-5.9` still has old version
3. ❌ `compinit -C` loads from cache (ignores new file)
4. ❌ Dynamic file tries to override cached (old) version
5. ❌ Result: Broken completions

### What We Need

**Option 1: Invalidate Cache on Install**
```bash
# In justfile, after regenerating completions:
rm -f ~/.cache/zsh/.zcompdump-*
```

**Option 2: Force Cache Rebuild**
```bash
# In justfile, after regenerating completions:
zsh -c 'autoload -Uz compinit && compinit -d ~/.cache/zsh/.zcompdump-$ZSH_VERSION'
```

**Option 3: Don't Use Cache for nabi Completions**
- Load dynamic file BEFORE compinit
- But this breaks the "load after compinit" pattern

---

## The Root Cause: Timing Dependency

### Current Flow (BROKEN)

```
Shell Startup:
  1. compinit -C (loads from cache, might be stub or function)
  2. Dynamic file sources (tries to override _nabi)
  3. ❌ Can't predict what _nabi is (stub vs function)

just install:
  1. Regenerates ~/.zsh/completions/_nabi
  2. ❌ Cache still has old version
  3. Next shell: compinit -C loads old cached version
  4. Dynamic file overrides old cached version
  5. ❌ MISMATCH: File has new code, cache has old code
```

### Why This Is Fragile

1. **Cache timing**: 20-hour window means cache might be fresh or stale
2. **Cache contents**: Might have stub or function body (unpredictable)
3. **File changes**: `just install` updates file but not cache
4. **Override timing**: Dynamic file runs after compinit, but compinit might have cached version

---

## The Solution: Fix The Design, Not The Cache

### Why Caching Isn't The Real Problem

**The cache is working correctly!** The problem is:
- We're trying to override a function that might be cached
- We're copying a function that might be a stub
- We're depending on timing (when cache was built, what it contains)

### The Real Fix: Don't Override `_nabi` At All

Instead of:
```zsh
# ❌ Override main dispatcher (fragile)
_nabi() {
    # ... check context ...
    _nabi_original "$@"
}
```

Do this:
```zsh
# ✅ Define specific completion functions (stable)
_nabi__tmux__send_prompt_pane() {
    _nabi_tmux_send_prompt_pane
}
```

**Benefits**:
- No overriding needed
- No copying functions
- No autoload stub issues
- Works with cache or without cache
- Stable and predictable

---

## Cache Behavior Summary

### What `compinit -C` Does

- **Loads from cache only** (doesn't check files)
- **Fast** (skips file system checks)
- **Might be stale** (if files changed but cache didn't)

### What `compinit` Does

- **Rebuilds cache** (checks all files)
- **Slower** (scans file system)
- **Always fresh** (matches current files)

### Your Configuration

- **Cache fresh (< 20 hours)**: Uses `compinit -C` (fast, might be stale)
- **Cache stale (> 20 hours)**: Uses `compinit` (slow, always fresh)

### The Problem

- **`just install` updates file** but doesn't invalidate cache
- **Next shell uses `compinit -C`** (cache is fresh, < 20 hours)
- **Loads old cached version** (doesn't see new file)
- **Dynamic file overrides old cached version** (mismatch!)

---

## Recommendations

### Short-Term Fix

**Invalidate cache on install**:
```bash
# In justfile, after regenerating completions:
rm -f "${HOME}/.cache/zsh/.zcompdump-${ZSH_VERSION}"
```

**Or force rebuild**:
```bash
# In justfile, after regenerating completions:
zsh -c "autoload -Uz compinit && compinit -d ${HOME}/.cache/zsh/.zcompdump-\${ZSH_VERSION}"
```

### Long-Term Fix

**Don't override `_nabi`**:
1. Post-process generated file to replace `:` with specific functions
2. Define those functions in dynamic file
3. No overriding, no copying, no cache issues

---

## Summary

**Your caching is correct!** The problem is:
1. ✅ Cache works as designed (performance optimization)
2. ❌ We're overriding a function that might be cached
3. ❌ Cache might have stub or function (unpredictable)
4. ❌ `just install` updates file but not cache
5. ❌ Next shell loads old cached version

**The fix**: Don't fight the cache. Fix the design to not override `_nabi` at all.
