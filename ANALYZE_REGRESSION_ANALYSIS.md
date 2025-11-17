# Analyze Repo - Cache Coherency Issues Analysis

## Executive Summary

The regression test failures in `tests/analyze_regression.sh` reveal a fundamental **cache directory naming mismatch** between what the implementation creates and what the test expects. The implementation uses a flat structure (`repo-name/`) while tests expect a per-language, hash-based structure (`repo-name-hash-language/`).

**Failing Scenarios:**
- **Scenario 9 (Concurrent Analysis)**: Concurrent writes to same cache directory cause race conditions
- **Scenario 10 (Command Equivalence)**: Subcommand cache reuse not working due to path mismatch

## Root Cause Analysis

### Issue 1: Language-Agnostic Cache Directory

**Current Implementation** (`src/repo/analyze.rs:56-60`):
```rust
let graphs_dir = codegraph::get_graphs_dir()?;
let index_dir = graphs_dir.join(repo_name);  // ❌ No language suffix
let graph_file = index_dir.join("graph.json");
```

**Current Implementation** (`src/repo/codegraph.rs:253-265`):
```rust
pub fn generate_index(repo_path: &str, _language: &str) -> Result<CodegraphIndex> {
    // ...
    let graphs_dir = get_graphs_dir()?;
    let output_dir = graphs_dir.join(repo_name);  // ❌ No language suffix
    // ...
}
```

**Test Expectation** (`tests/analyze_regression.sh:154-176`):
```bash
# Look for exact match: repo-hash-lang
local found=$(find "$CACHE_DIR" -maxdepth 1 -type d -name "${repo_name}-*-${lang}" 2>/dev/null | head -1)
```

**Impact:**
- `nabi analyze repo . --lang python` → creates `~/.local/state/nabi/codegraph/graphs/test-repo/`
- `nabi analyze repo . --lang rust` → **overwrites** the same directory
- Different languages cannot coexist, violating Scenario 3 (Language Separation) and Scenario 9 (Concurrent Analysis)

### Issue 2: Missing Hash in Cache Directory Name

The test expects deterministic hashing of the repository path to create unique cache keys:

**Pattern Expected:** `{repo-name}-{8-char-hash}-{language}`
- Example: `test-repo-a1b2c3d4-python`
- Example: `test-repo-a1b2c3d4-rust`

**Current Pattern:** `{repo-name}`
- Example: `test-repo` (all analyses regardless of path or language)

**Why This Matters:**
- Allows analyzing the same repo name from different paths
- Deterministically reproducible cache locations
- Prevents cache collisions across different repos with same name

### Issue 3: Language Parameter Not Used

The `_language: &str` parameter in `generate_index` is **intentionally prefixed with `_`** (line 253), indicating it's not used:

```rust
pub fn generate_index(repo_path: &str, _language: &str) -> Result<CodegraphIndex> {
    // language parameter is ignored
}
```

This means:
- Language detection is silently ignored
- No per-language cache differentiation
- `--lang` CLI flag has no effect on cache location

### Issue 4: load_index Function Doesn't Receive Language

The `load_index` function (line 337 in codegraph.rs) takes only `repo_name`:

```rust
pub fn load_index(repo_name: &str) -> Result<CodegraphIndex> {
    let graphs_dir = get_graphs_dir()?;
    let graph_dir = graphs_dir.join(repo_name);  // ❌ Assumes single cache
```

But in `analyze.rs:69`, the language is not passed to load_index:

```rust
let cached_index = codegraph::load_index(repo_name)?;  // ❌ Language lost
```

## Affected Test Scenarios

| Scenario | Status | Why Failing | Impact |
|----------|--------|-----------|--------|
| 1 - Cache Reuse | ✅ PASS | Not language-dependent | Works by luck |
| 2 - Force Rebuild | ✅ PASS | Not language-dependent | Works by luck |
| 3 - Language Separation | ❌ FAIL | Same dir for all languages | Rust overwrites Python |
| 4 - Graph Query Resolution | ✅ PASS | Loads any cache found | Doesn't check language |
| 5 - Multi-Agent Sharing | ✅ PASS | Not language-dependent | Works by luck |
| 6 - Cache Directory Naming | ✅ PASS | No hash validation | Doesn't check format |
| 7 - Missing Cache Handling | ✅ PASS | Generic error | Works by luck |
| 8 - Corrupted Cache | ✅ PASS | Generic error | Works by luck |
| 9 - Concurrent Analysis | ❌ FAIL | Race condition on same dir | Both write to `test-repo/` |
| 10 - Command Equivalence | ❌ FAIL | Subcommand path mismatch | Different routing paths |

## Implementation Plan

### Phase 1: Hash-Based Cache Directory

Add hash computation function in `codegraph.rs`:

```rust
use sha2::{Sha256, Digest};
use hex;

/// Compute deterministic hash of repository path
fn compute_repo_hash(repo_path: &str) -> String {
    let path = std::path::Path::new(repo_path)
        .canonicalize()
        .unwrap_or_else(|_| std::path::PathBuf::from(repo_path));

    let canonical = path.to_string_lossy();
    let mut hasher = Sha256::new();
    hasher.update(canonical.as_bytes());
    let hash = hasher.finalize();
    // Return first 8 chars of hex hash
    format!("{}", hex::encode(&hash[..4]))
}
```

### Phase 2: Language-Aware Cache Paths

Modify `generate_index` signature and implementation:

```rust
pub fn generate_index(repo_path: &str, language: &str) -> Result<CodegraphIndex> {
    let path = Path::new(repo_path).canonicalize()?;
    let repo_name = path.file_name().and_then(|n| n.to_str()).unwrap_or("unknown");
    let repo_hash = compute_repo_hash(repo_path);

    // Format: repo-name-hash-language
    let cache_dir_name = format!("{}-{}-{}", repo_name, repo_hash, language);

    let graphs_dir = get_graphs_dir()?;
    let output_dir = graphs_dir.join(&cache_dir_name);
    // ...
}
```

### Phase 3: Thread-Safe Cache Loading

Modify `load_index` to accept language and use hash:

```rust
pub fn load_index(repo_path: &str, language: &str) -> Result<CodegraphIndex> {
    let path = Path::new(repo_path).canonicalize()?;
    let repo_name = path.file_name().and_then(|n| n.to_str()).unwrap_or("unknown");
    let repo_hash = compute_repo_hash(repo_path);

    let cache_dir_name = format!("{}-{}-{}", repo_name, repo_hash, language);
    let graphs_dir = get_graphs_dir()?;
    let graph_dir = graphs_dir.join(&cache_dir_name);
    // ...
}
```

### Phase 4: Update analyze.rs to Pass Language

```rust
let cached_index = codegraph::load_index(repo_path, &detected_lang)?;
```

### Phase 5: Add Atomic Write Protection

Use temp files + atomic rename to prevent concurrent corruption:

```rust
let output_dir_temp = graphs_dir.join(format!("{}.tmp", cache_dir_name));
// ... write to temp ...
// Atomic rename
std::fs::rename(&output_dir_temp, &output_dir)?;
```

## Files to Modify

1. **src/repo/codegraph.rs**
   - Add `compute_repo_hash()` function
   - Modify `generate_index()` to use language and hash in path
   - Modify `load_index()` to accept language and use hash

2. **src/repo/analyze.rs**
   - Pass `&detected_lang` to `codegraph::load_index()`
   - Add debug output showing cache directory (for test debugging)

3. **Cargo.toml**
   - Verify `sha2` and `hex` dependencies exist (already present)

## Testing Strategy

Run regression tests after each phase:

```bash
# Full test suite
cargo test --release

# Analyze regression tests only
./tests/analyze_regression.sh --verbose

# Specific scenario
./tests/analyze_regression.sh --scenario concurrent --verbose
./tests/analyze_regression.sh --scenario command_equivalence --verbose
```

## Backward Compatibility

⚠️ **Breaking Change:**

Existing caches at `~/.local/state/nabi/codegraph/graphs/test-repo/` will not be found after this fix. Users must re-run analysis:

```bash
# Clean old caches
rm -rf ~/.local/state/nabi/codegraph/graphs/*

# Re-generate
nabi analyze repo . --lang python
```

This is acceptable because:
1. Codegraph is still in development (pre-1.0)
2. Cache regeneration is fast (typically <1 second)
3. No persistent user data stored

## Summary

| Problem | Root Cause | Solution | Effort |
|---------|-----------|----------|--------|
| Language mixing | No language in path | Add language suffix | 1-2 hours |
| Cache collisions | No hash | Add repo hash | 1 hour |
| Concurrent races | Same output dir | Atomic temp + rename | 1 hour |
| Path issues | Hard-coded names | Use canonicalize + hash | Already in place |

**Total Estimated Effort:** 3-4 hours for complete fix + testing
