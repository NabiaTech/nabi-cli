# TMUX List Commands — Test & Expectations (Spec v2)

Purpose: Provide a clear, repeatable test plan and expected behaviors for the enhanced `nabi tmux list` commands (sessions, windows, panes) with explicit invocation context, strict/non‑strict modes, and backward‑compatible formats.

Status: Locked Spec. Use this as the acceptance checklist when implementing or validating.

## Scope

Commands under test:
- `nabi tmux list sessions` (formats: `json`, `json+context`)
- `nabi tmux list windows [SESSION|-s/--session <SESSION>]` (formats: `json`, `json+context`; strict by default)
- `nabi tmux list panes [TARGET|-s/--session <SESSION> -w/--window <INDEX|:={NAME}>]` (formats: `json`, `json+context`; strict by default)

Out of scope: send‑prompt execution semantics (covered separately), pane capture/streaming.

## Exit Codes

- 0: Success (including non‑strict with mismatch)
- 1: General errors (bad args, tmux not found, not in tmux with no session, ambiguous name, etc.)
- 2: Context mismatch in strict mode (expected session differs from current)

## Strictness Controls

- Default: strict (mismatch → exit 2)
- Non‑strict: `--no-strict` flag OR env `NABI_TMUX_STRICT=0` (warn to stderr, set `mismatch: true`, exit 0)

## Indexing Rules

- User‑facing indices mirror tmux base settings (no remapping).
- Include both bases in `invocation_context`:
  - `window_index_base`: 0 or 1
  - `pane_index_base`: 0 or 1

## Context Source

- Context (session/window/pane) reflects the current tmux client invoking the command.
- In nested/multi‑client situations, this is the caller’s context (documented limitation).

## Output Formats

- `--format json`: Backward‑compatible arrays (no wrapper object).
- `--format json+context`: Object wrapper with `version: 2`, `invocation_context`, and the items array.

### ContextInfo fields (json+context)

```
invocation_context: {
  in_tmux: bool,
  session_name: string|null,
  window_index: number|null,
  window_name: string|null,
  pane_index: number|null,
  window_index_base: 0|1,
  pane_index_base: 0|1,
  mismatch: bool (windows/panes only, present when non‑strict and mismatch)
}
```

## Test Environment Assumptions

- tmux is installed and a tmux server is running.
- You have at least one session, e.g., `alignment`, with windows and panes similar to examples.
- You can run tests both inside a tmux client (attached) and outside tmux (detached shell).

Tip: When testing outside tmux, `in_tmux` should be false, and commands that rely on current session must error unless `--session` is provided.

---

## Test Matrix

### A. Sessions

1) Backward‑compatible JSON array

Command:
```
nabi tmux list sessions --format json
```
Expect:
- Exit 0
- Stdout: JSON array of sessions (unchanged shape), e.g.:
```
[
  { "name": "alignment", "windows": 9, "attached": true, "created": "...", "is_claude_tui": true }
]
```

2) JSON with context

Command (inside tmux, current session is `alignment`, window index 8, pane 1):
```
nabi tmux list sessions --format json+context | jq '.'
```
Expect:
- Exit 0
- Stdout object contains:
  - `version: 2`
  - `invocation_context.in_tmux: true`
  - `invocation_context.session_name: "alignment"`
  - `invocation_context.window_index_base` and `pane_index_base` present (0 or 1)
  - `sessions` array as in JSON format (preserve `is_claude_tui`)

3) Outside tmux

Command (outside tmux):
```
nabi tmux list sessions --format json+context
```
Expect:
- Exit 0
- `invocation_context.in_tmux: false`
- Bases may default to 0 if not retrievable; context indices are null.

### B. Windows

4) Default: current session (inside tmux)

Command (inside tmux, current session `alignment`):
```
nabi tmux list windows --format json
```
Expect:
- Exit 0
- JSON array items with fields: `index`, `name`, `pane_count`, `is_active`.

5) Explicit session (strict, matching current)

```
nabi tmux list windows alignment --format json+context | jq '.'
```
Expect:
- Exit 0; `invocation_context.session_name == "alignment"`
- `invocation_context.mismatch: false`
- `windows` array with expected fields

6) Explicit session mismatch (strict by default)

Assume current session is `alignment`, but query `meta`:
```
nabi tmux list windows meta --format json+context
echo $?  # should be 2
```
Expect:
- Exit 2
- Stderr: error explaining mismatch with current context
- No JSON (or a minimal error JSON if implementation chooses), but test expects non‑zero exit.

7) Explicit session mismatch (non‑strict)

```
nabi tmux list windows meta --format json+context --no-strict | jq '.invocation_context.mismatch'
```
Expect:
- Exit 0
- Stderr: warning describing mismatch
- JSON present, `invocation_context.mismatch == true`

8) Not in tmux and no session provided

```
tmux detach-client  # run from a non‑tmux shell
nabi tmux list windows --format json
echo $?
```
Expect:
- Exit 1
- Stderr: guidance to specify `--session` and list available sessions

### C. Panes

9) Target by index (current session)

```
nabi tmux list panes 8 --format json
```
Expect:
- Exit 0
- Array of panes with fields: `pane`, `is_active`, `pane_id`, `window_id`, `send_prompt_target`, `tmux_target`, `target_shorthand`, `target_minimal`
- If `pane_count == 1`, `target_minimal` should be `:8`; else `:8.X`

10) Target by full spec (session:window)

```
nabi tmux list panes alignment:8 --format json+context | jq '.'
```
Expect:
- Exit 0
- `invocation_context.session_name` present; `mismatch` false when session matches current (strict)
- Panes array as above

11) Target via flags (session+window index)

```
nabi tmux list panes -s alignment -w 8 --format json
```
Expect:
- Equivalent to `alignment:8`

12) Target by exact name (name resolution required syntax)

```
nabi tmux list panes -s alignment -w ':={fix-kg}' --format json
```
Expect:
- Exit 0
- Resolves `fix-kg` to a single window index; errors if ambiguous (see next)

13) Ambiguous window name

Create duplicate window names or simulate:
```
nabi tmux list panes -s alignment -w ':={repo-check}' --format json
echo $?
```
Expect:
- Exit 1
- Stderr: "Ambiguous window name 'repo-check': found windows [1, 3]. Use window index or :={name} syntax."

14) Mismatch strict vs non‑strict

```
# Strict (default):
nabi tmux list panes -s meta -w 1 --format json+context; echo $?

# Non‑strict:
nabi tmux list panes -s meta -w 1 --no-strict --format json+context | jq '.invocation_context.mismatch'
```
Expect:
- Strict: exit 2; Non‑strict: exit 0 with `mismatch: true` and warning to stderr

### D. Index Base Verification

15) Index bases reported correctly

```
nabi tmux list windows alignment --format json+context | jq '.invocation_context | {window_index_base, pane_index_base}'
```
Expect:
- Values match `tmux show-options -gv window-base-index` and `pane-base-index`
- Item indices (windows/panes) mirror tmux base (no remapping)

---

## Example Outputs (Illustrative)

Sessions (json):
```
[
  { "name": "alignment", "windows": 9, "attached": true, "created": "2025-11-05T14:57:00+0000", "is_claude_tui": true }
]
```

Windows (json+context):
```
{
  "version": 2,
  "invocation_context": {
    "in_tmux": true,
    "session_name": "alignment",
    "window_index": 8,
    "window_name": "fix-kg",
    "pane_index": 1,
    "window_index_base": 0,
    "pane_index_base": 0,
    "mismatch": false
  },
  "windows": [
    { "index": 8, "name": "fix-kg", "pane_count": 4, "is_active": true }
  ]
}
```

Panes (json):
```
[
  {
    "pane": 1,
    "is_active": true,
    "pane_id": "%0",
    "window_id": "@0",
    "send_prompt_target": "alignment:8.1",
    "tmux_target": "alignment:8.1",
    "target_shorthand": ":8.1",
    "target_minimal": ":8"
  }
]
```

---

## Negative/Edge Tests

- Not in tmux and no session flag (windows/panes): exit 1 with guidance and available sessions listed.
- Unknown session: exit 1 with available sessions.
- Ambiguous window name with `:={name}` resolving to multiple indices: exit 1 with list of indices.
- Rapid state changes: indices may shift; `pane_id`/`window_id` must be included and stable.

---

## Agent Consumption Guidance

- Prefer `--format json+context` to obtain `invocation_context` and `mismatch` flag.
- Use `send_prompt_target` for `nabi tmux send-prompt`; use `tmux_target` for raw `tmux send-keys`.
- `target_minimal` is only safe when `pane_count == 1`; otherwise specify `.pane`.
- If strict mismatch occurs (exit 2), self‑correct by re‑resolving session via `list sessions` then retry with explicit `--session`.

---

## Acceptance Checklist

- [ ] Sessions: `json` unchanged; `json+context` includes `version: 2` and full context.
- [ ] Windows: Supports positional/flag session; includes `pane_count`, `is_active`; enforces strictness; `json+context` includes `mismatch` and bases.
- [ ] Panes: Accepts legacy target or `-s/-w`; supports `:={name}` exact match; includes stable IDs and all target variants; enforces strictness.
- [ ] Exit codes: 0/1/2 as specified.
- [ ] stderr carries warnings in non‑strict mode; stdout is strict JSON only.
- [ ] Indices mirror tmux bases; bases reported in context.

---

## Notes / Future

- Consider accepting `%pane_id` and `@window_id` as future `send-prompt` targets.
- Optional `--with-context` alias for `--format json+context` (ergonomic sugar).

