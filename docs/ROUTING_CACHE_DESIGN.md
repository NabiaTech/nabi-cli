# Commander Routing Cache Design Decision

**Date**: 2025-01-10
**Context**: Optimization exploration for `route_to_commander()` function
**Decision**: No caching implemented (documentation-only optimization notes)

---

## Problem Statement

The `route_to_commander()` function performs file system checks on every invocation:
1. Check if native Rust commander binary exists: `~/.config/nabi/commanders/{commander}/{commander}`
2. Fallback check if Python CLI exists: `~/.local/bin/nabi-python`

**Question**: Can we cache these routing decisions to avoid repeated `stat()` system calls?

---

## Synchronization Primitive Options

### Option 1: `OnceLock<HashMap>` (Immutable Cache)

**What it is**: A thread-safe, one-time initialization primitive. Perfect for static configuration that never changes.

**Code attempt**:
```rust
use std::sync::OnceLock;
use std::collections::HashMap;

static COMMANDER_CACHE: OnceLock<HashMap<String, CommanderRoute>> = OnceLock::new();

fn get_commander_route(commander: &str) -> Result<CommanderRoute> {
    let cache = COMMANDER_CACHE.get_or_init(|| HashMap::new());

    // ❌ PROBLEM: Can't insert into HashMap after initialization
    if let Some(route) = cache.get(commander) {
        return Ok(route.clone());
    }

    // This would fail - OnceLock doesn't allow mutation
    cache.insert(commander.to_string(), route); // ❌ Compile error
}
```

**Why it fails**:
- `OnceLock` is designed for **immutable** data that's initialized once
- The HashMap is created empty, but we can't add entries after initialization
- This pattern works for static config, not dynamic per-commander caching

**When to use**: Static configuration, computed constants, lazy initialization of immutable data.

---

### Option 2: `Mutex<HashMap>` (Mutable Cache with Locking)

**What it is**: A mutual exclusion lock that allows safe mutation of shared data.

**How it would work**:
```rust
use std::sync::Mutex;
use std::collections::HashMap;

static COMMANDER_CACHE: Mutex<HashMap<String, CommanderRoute>> =
    Mutex::new(HashMap::new());

fn get_commander_route(commander: &str) -> Result<CommanderRoute> {
    // Lock the cache for reading
    let cache = COMMANDER_CACHE.lock().unwrap();

    // Check cache
    if let Some(route) = cache.get(commander) {
        return Ok(route.clone());
    }

    // Drop lock before file system check (avoid holding lock during I/O)
    drop(cache);

    // Perform file system check
    let route = check_file_system(commander)?;

    // Lock again for writing
    let mut cache = COMMANDER_CACHE.lock().unwrap();
    cache.insert(commander.to_string(), route.clone());

    Ok(route)
}
```

**Pros**:
- ✅ Works for dynamic caching
- ✅ Thread-safe mutation
- ✅ Simple API

**Cons**:
- ❌ **Lock contention**: Every call acquires exclusive lock (even reads)
- ❌ **Deadlock risk**: Must be careful about lock ordering
- ❌ **Performance overhead**: Lock acquisition has CPU cost (~10-50ns per lock)
- ❌ **Complexity**: Need to manage lock scope carefully

**Performance impact**:
- Lock acquisition: ~10-50ns
- File system check: ~2ms (2000x slower than lock)
- **Verdict**: Lock overhead is negligible compared to file I/O, but adds complexity

---

### Option 3: `RwLock<HashMap>` (Read-Write Lock)

**What it is**: Allows multiple concurrent readers OR one exclusive writer.

**How it would work**:
```rust
use std::sync::RwLock;
use std::collections::HashMap;

static COMMANDER_CACHE: RwLock<HashMap<String, CommanderRoute>> =
    RwLock::new(HashMap::new());

fn get_commander_route(commander: &str) -> Result<CommanderRoute> {
    // Try read lock first (allows concurrent readers)
    {
        let cache = COMMANDER_CACHE.read().unwrap();
        if let Some(route) = cache.get(commander) {
            return Ok(route.clone());
        }
    } // Lock released here

    // File system check (no lock held)
    let route = check_file_system(commander)?;

    // Write lock (exclusive)
    {
        let mut cache = COMMANDER_CACHE.write().unwrap();
        // Double-check after acquiring write lock (another thread might have inserted)
        cache.entry(commander.to_string())
            .or_insert_with(|| route.clone());
    }

    Ok(route)
}
```

**Pros**:
- ✅ **Better concurrency**: Multiple readers can access cache simultaneously
- ✅ **Optimized for read-heavy workloads**: Perfect for caching scenarios
- ✅ **Thread-safe**: Handles concurrent access correctly

**Cons**:
- ❌ **More complex**: Read vs write lock distinction requires careful design
- ❌ **Writer starvation**: If many readers hold locks, writers wait
- ❌ **Still has overhead**: Lock acquisition still costs CPU cycles
- ❌ **Double-check pattern needed**: Race condition between read unlock and write lock

**Performance impact**:
- Read lock: ~10-30ns (slightly faster than Mutex)
- Write lock: ~20-50ns (similar to Mutex)
- File system check: ~2ms (still dominates)

---

## Performance Analysis

### Current Implementation (No Cache)
```
Command: nabi docs manifest list
├─ route_to_commander("docs")
│  ├─ Check native binary: ~2ms (file system stat)
│  └─ Check Python CLI: ~2ms (file system stat)
└─ Total overhead: ~4ms per call
```

### With Mutex/RwLock Cache (Hypothetical)
```
First call:
├─ Lock acquisition: ~30ns
├─ Cache miss
├─ File system check: ~2ms
├─ Lock acquisition: ~30ns
├─ Cache insert: ~100ns
└─ Total: ~2.1ms

Subsequent calls:
├─ Lock acquisition: ~30ns
├─ Cache hit: ~50ns (HashMap lookup)
└─ Total: ~80ns (25x faster!)
```

**But**: CLI tools are typically single-threaded, sequential commands. The concurrency benefits of `RwLock` don't apply here.

---

## Decision Rationale

### Why We Chose NOT to Implement Caching

1. **Single-threaded context**: CLI commands run sequentially, not concurrently
   - No benefit from `RwLock`'s concurrent read optimization
   - `Mutex` would work but adds complexity for minimal gain

2. **File system checks are fast**: ~2ms is acceptable for CLI tool startup
   - The real bottleneck is bash script parsing (~10-15ms), not file checks
   - Optimizing the wrong thing doesn't help

3. **Complexity vs benefit**:
   - Cache implementation: ~50 lines of code, lock management, error handling
   - Performance gain: ~2ms saved per call (only on cache hits)
   - **ROI**: Low - complexity added doesn't justify the small speedup

4. **Cache invalidation complexity**:
   - What if commander binary is installed/uninstalled during runtime?
   - Need cache invalidation logic
   - Adds more complexity

5. **Process lifetime**:
   - CLI tools spawn new processes per command
   - Cache would only help within a single process
   - Most commands are one-shot (no repeated calls)

---

## When Caching WOULD Make Sense

### Scenario 1: Long-running daemon/service
```rust
// If nabi-cli ran as a daemon handling multiple requests
// Then caching would be valuable:
static COMMANDER_CACHE: RwLock<HashMap<String, CommanderRoute>> =
    RwLock::new(HashMap::new());
```

### Scenario 2: Batch operations
```rust
// If we processed many commands in a single process:
nabi docs manifest list
nabi docs manifest validate repo1
nabi docs manifest validate repo2  // Cache hit!
```

### Scenario 3: Very slow file system checks
```rust
// If file checks took 100ms+ (network filesystem, etc.)
// Then caching overhead becomes negligible
```

---

## Alternative Optimizations (Better ROI)

Instead of caching, consider:

### 1. Direct Routing (Skip Shim)
```rust
// Skip nabi-python entirely for known commanders
if commander == "docs" {
    return route_docs_directly(args)?;
}
```
**Benefit**: Saves ~10-15ms (bash script parsing overhead)
**Complexity**: Medium (need to locate implementation)

### 2. Native Rust Implementation
```rust
// Implement manifest commands in Rust
// Eliminates all shim overhead
```
**Benefit**: Saves ~60ms+ (all Python/bash overhead)
**Complexity**: High (requires implementing logic)

### 3. Optimize Bash Script
```rust
// Reduce nabi-python script overhead
// Use exec more, reduce parsing
```
**Benefit**: Saves ~5-10ms
**Complexity**: Low (script optimization)

---

## Summary Table

| Approach | Works? | Complexity | Performance Gain | Verdict |
|----------|--------|------------|------------------|---------|
| `OnceLock<HashMap>` | ❌ No | Low | N/A | Can't mutate after init |
| `Mutex<HashMap>` | ✅ Yes | Medium | ~2ms per cache hit | Overkill for CLI |
| `RwLock<HashMap>` | ✅ Yes | High | ~2ms per cache hit | Overkill for CLI |
| **No caching** | ✅ Yes | **Low** | N/A | **✅ Chosen** |

---

## References

- [Rust `OnceLock` docs](https://doc.rust-lang.org/std/sync/struct.OnceLock.html)
- [Rust `Mutex` docs](https://doc.rust-lang.org/std/sync/struct.Mutex.html)
- [Rust `RwLock` docs](https://doc.rust-lang.org/std/sync/struct.RwLock.html)
- [Rust Book: Shared-State Concurrency](https://doc.rust-lang.org/book/ch16-03-shared-state.html)

---

## Conclusion

**Decision**: Document optimization potential but don't implement caching.

**Reasoning**:
- File system checks are fast enough (~2ms)
- CLI tools are single-threaded (no concurrency benefit)
- Complexity doesn't justify small performance gain
- Better optimizations exist (direct routing, native Rust)

**Future consideration**: If nabi-cli becomes a long-running service or handles batch operations, revisit caching with `RwLock<HashMap>`.
