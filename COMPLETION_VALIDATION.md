# Completion Composition Validation

## Overview

The completion system uses **build-time validation** to ensure the merged completion file remains valid and intact. This prevents breakage when either the static (clap-generated) or dynamic (tmux enhancements) components change.

**Why this matters**: Completions are complex zsh functions that are brittle to structural changes. A single misplaced line can break the entire completion system. This validation catches errors at build time, not at runtime.

---

## Architecture

```
┌─────────────────────────────────────────────────────────┐
│                    make install                         │
└────────────────┬────────────────────────────────────────┘
                 │
                 ▼
        ┌─────────────────────┐
        │   cargo build       │
        └────────┬────────────┘
                 │
                 ▼
        ┌─────────────────────────────────────┐
        │ Generate static completion from clap│
        │  (.build/_nabi_static)              │
        └────────┬────────────────────────────┘
                 │
                 ▼
        ┌──────────────────────────────────────────┐
        │ Merge with dynamic enhancements          │
        │ contrib/nabi-completions-dynamic.zsh     │
        │  → ~/.cache/zsh/completions/_nabi       │
        └────────┬─────────────────────────────────┘
                 │
                 ▼
        ┌───────────────────────────────────────┐
        │ Validate composition                  │
        │ scripts/validate-completions.sh       │
        │ ✓ Syntax checks (bash -n)            │
        │ ✓ Structural integrity                │
        │ ✓ Dynamic enhancements injected       │
        └────────┬────────────────────────────┘
                 │
         ┌───────┴───────┐
         │               │
      PASS          FAIL (exit 2)
         │               │
         ▼               ▼
      Install        Build aborted
      Binary      Error message
```

---

## Validation Checks

### Critical Checks (Abort Build on Failure)

1. **Static syntax**: `bash -n .build/_nabi_static`
   - Ensures clap-generated completion is valid zsh

2. **Dynamic syntax**: `bash -n contrib/nabi-completions-dynamic.zsh`
   - Ensures dynamic enhancements are valid zsh

3. **Merged syntax**: `bash -n ~/.cache/zsh/completions/_nabi`
   - Ensures composition didn't break zsh syntax

4. **Single #compdef line**: Only one at start of file
   - zsh completion header must appear exactly once

5. **Dynamic injection marker**: Line 6677+ must contain injection marker
   - Ensures dynamic code was merged correctly

6. **Hook mechanism intact**: Override functions present
   - Dynamic interception depends on hook mechanism

7. **Helper functions present**: Both `_nabi_get_tmux_sessions` and `_nabi_tmux_send_prompt_pane`
   - Dynamic tmux completion requires these functions

### Non-Critical Checks (Warnings Only)

8. **File size**: Should be 150-500KB
   - Sanity check for suspiciously small/large files

9. **No duplicates**: AWK check for duplicate function definitions
   - Would indicate merge went wrong

10. **Static preserved**: Contains `autoload -U is-at-least`
    - Spot-check that base completion wasn't lost

---

## Running Validation

### Automatic (via make)

```bash
cd ~/nabia/core/nabi-cli
make install
# Automatically runs validation after composing completions
```

### Manual (for debugging)

```bash
# Run validation only (doesn't rebuild)
bash scripts/validate-completions.sh

# Verbose output
bash scripts/validate-completions.sh --verbose  # Note: --verbose not yet implemented

# Rebuild and validate
make completions
```

---

## Output Formats

### Success

```
━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━
Completion Composition Validator
━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━

Checking prerequisites...
→ Found: _nabi_static (6675 lines)
→ Found: nabi-completions-dynamic.zsh (238 lines)
→ Found: _nabi (6914 lines)
✓ All files exist

Critical checks:
✓ Static completion has valid zsh syntax
✓ Dynamic completion has valid zsh syntax
✓ Merged output has valid zsh syntax
✓ Exactly one #compdef directive (at start)
✓ Dynamic injection marker present
✓ Dynamic override mechanism intact
✓ Dynamic helper functions present

Non-critical checks:
✓ Output file size is reasonable (204KB)
✓ No duplicate function definitions
✓ Static completion base preserved

━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━
Results: 11 passed, 0 failed, 0 warnings
━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━

✓ All validation checks PASSED
```

### Failure Example

```
✗ Dynamic injection marker MISSING - merge may have failed
✗ Validation FAILED - build aborted
```

---

## When Validation Fails

### Common Issues

**Problem**: `Dynamic injection marker MISSING`
- **Cause**: Makefile composition didn't complete or dynamic file wasn't found
- **Fix**: Check that `contrib/nabi-completions-dynamic.zsh` exists and is readable
- **Action**: Run `make clean && make install`

**Problem**: `Merged output has INVALID zsh syntax`
- **Cause**: Injection corrupted the syntax (rare)
- **Fix**: Check diff between static and output
- **Action**: `diff .build/_nabi_static ~/.cache/zsh/completions/_nabi | head -50`

**Problem**: `Dynamic override mechanism MISSING`
- **Cause**: `contrib/nabi-completions-dynamic.zsh` doesn't have override code
- **Fix**: Verify dynamic file hasn't been accidentally modified
- **Action**: `grep -c "if (( \$+functions\[_nabi\] ))" contrib/nabi-completions-dynamic.zsh`

### Debug Steps

1. **Check files exist**:
   ```bash
   ls -lh .build/_nabi_static
   ls -lh contrib/nabi-completions-dynamic.zsh
   ls -lh ~/.cache/zsh/completions/_nabi
   ```

2. **Check syntax individually**:
   ```bash
   bash -n .build/_nabi_static && echo "Static OK"
   bash -n contrib/nabi-completions-dynamic.zsh && echo "Dynamic OK"
   bash -n ~/.cache/zsh/completions/_nabi && echo "Merged OK"
   ```

3. **Check injection point**:
   ```bash
   grep -n "Dynamic tmux completion enhancements" ~/.cache/zsh/completions/_nabi
   ```

4. **Look at line count**:
   ```bash
   wc -l .build/_nabi_static contrib/nabi-completions-dynamic.zsh ~/.cache/zsh/completions/_nabi
   # Output should show: _nabi = _nabi_static + (dynamic - 1)
   # Because we skip the #compdef line in dynamic
   ```

---

## For Developers

### When to Expect Validation to Run

- **Always**: Every `make install`
- **Always**: Every `make completions`
- **Never**: `cargo build` alone (validation is in Makefile, not build.rs)

### How to Modify Components Safely

**Changing static completion (generated from clap)**:
- Edit your CLI code in `src/cli.rs`
- Run `make install` → new static completion generated automatically
- Validation ensures syntax is still valid

**Changing dynamic enhancements**:
- Edit `contrib/nabi-completions-dynamic.zsh`
- Run `make install` → dynamic enhancements re-injected
- Validation ensures merger didn't break anything

**Testing without full build**:
```bash
# Just validate current state (no recompile)
bash scripts/validate-completions.sh

# Compose completions without Rust build
# (requires existing .build/_nabi_static)
make completions --no-deps
```

### Schema Reference

The validation schema is defined in:
```
~/.config/nabi/cli/completion-composition.toml
```

This documents:
- Expected file locations
- Validation rules
- Integration method
- Recovery procedures

---

## Performance

- **Validation time**: ~50ms (just bash -n syntax checks)
- **Overhead in build**: Negligible (~0.1% of total build time)
- **Impact**: Fast feedback if composition breaks

---

## Future Enhancements

Potential improvements for when more infrastructure is in place:

1. **Integration with `nabi self validate`**: Add completion validation to CLI semantic
2. **Persistent validation log**: Track validation history
3. **Before/after comparison**: Diff report when composition changes
4. **Automated rollback**: Keep backup of last-known-good completion

---

## See Also

- [Makefile](./Makefile) - Build orchestration (lines 53-79)
- [contrib/nabi-completions-dynamic.zsh](./contrib/nabi-completions-dynamic.zsh) - Dynamic enhancements
- [~/.config/nabi/cli/completion-composition.toml](~/.config/nabi/cli/completion-composition.toml) - Schema
