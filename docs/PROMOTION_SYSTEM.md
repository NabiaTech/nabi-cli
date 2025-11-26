# Tool Promotion System

## Overview

The tool promotion system enables schema-driven deployment of tools from source to production locations in the XDG-compliant directory structure.

## Architecture

```
~/.config/nabi/tools/{tool_id}.toml   ← [promotion] config
          ↓
    nabi tool promote {tool_id}       ← CLI command
          ↓
~/.local/share/nabi/                   ← Deployed artifacts
~/.local/state/nabi/promoted/          ← Promotion records
```

## Promotion Modes

### LIVE Mode
**Use case**: Development, hot-reload scenarios
**Method**: Symlinks to source directory
**Version**: Always `live`
**Rollback**: Not supported (symlinks always point to latest)

```bash
nabi tool promote cursorignore --mode LIVE
```

### STABLE Mode (Recommended)
**Use case**: Production, versioned releases
**Method**: Immutable versioned copies
**Version**: Semantic versioning (e.g., `1.0.0`)
**Rollback**: Full support via version switching

```bash
nabi tool promote cursorignore --version 1.0.0 --mode STABLE
```

Directory structure:
```
~/.local/share/nabi/lib/
  ├── cursorignore@1.0.0/
  ├── cursorignore@1.0.1/
  └── cursorignore@latest → cursorignore@1.0.1
```

### INSTALL Mode
**Use case**: System-wide command integration
**Method**: Wrapper scripts in PATH
**Status**: Not yet implemented

## Configuration

Add a `[promotion]` section to your tool's TOML in `~/.config/nabi/tools/{tool_id}.toml`:

```toml
[promotion]
mode = "STABLE"
version = "1.0.0"
description = "Tool promotion config"

[promotion.artifacts.library]
source = "~/nabia/tools/my-tool/src/"
target = "~/.local/share/nabi/lib/my-tool@{version}/"
artifact_type = "library"

[promotion.artifacts.executable]
source = "~/nabia/tools/my-tool/my-tool.py"
target = "~/.local/share/nabi/bin/my-tool"
artifact_type = "binary"

[promotion.source]
repository = "~/nabia/tools/my-tool"
branch = "main"
```

### Artifact Types

| Type | Description | Use Case |
|------|-------------|----------|
| `library` | Code modules, packages | Importable Python/Rust libs |
| `binary` | Executables, scripts | CLI commands |
| `config` | Configuration files | Runtime configs |
| `hook` | Git hooks, automation scripts | Integration hooks |

### Version Template

Use `{version}` placeholder in target paths for versioned copies:
```toml
target = "~/.local/share/nabi/lib/tool@{version}/file.py"
```

This expands to:
- `~/.local/share/nabi/lib/tool@1.0.0/file.py`
- `~/.local/share/nabi/lib/tool@1.0.1/file.py`

## Promotion Records

Each promotion creates a record in `~/.local/state/nabi/promoted/{tool_id}.json`:

```json
{
  "record_id": "promo-20251124-143125-baf8d1fc",
  "tool_id": "cursorignore",
  "version": "1.0.0",
  "promoted_at": "2025-11-24T14:31:25Z",
  "mode": "STABLE",
  "artifacts": [...],
  "source": {
    "repository": "~/nabia/tools/cursorignore",
    "commit": "a3f8c9d",
    "branch": "main"
  },
  "version_aliases": ["latest"],
  "status": "active"
}
```

### Schema Compliance

Records validate against `~/.config/nabi/governance/schemas/promotion-record.schema.json`.

## CLI Usage

### Promote with default settings
```bash
nabi tool promote cursorignore
```

### Override version
```bash
nabi tool promote cursorignore --version 2.0.0
```

### Override mode
```bash
nabi tool promote cursorignore --mode LIVE
```

### Combine overrides
```bash
nabi tool promote cursorignore --version 1.5.0 --mode STABLE
```

## Version Aliasing

STABLE mode automatically creates version aliases (default: `latest`):

```toml
[promotion]
version_aliases = ["latest", "stable", "v1"]
```

Results in:
```
~/.local/share/nabi/lib/
  ├── tool@1.0.0/           # Versioned directory
  ├── tool@latest → tool@1.0.0
  ├── tool@stable → tool@1.0.0
  └── tool@v1 → tool@1.0.0
```

## Integration Points

### Federation Events
Promotions emit `tool.promoted` events to the federation event system (when enabled).

### XDG Compliance
All paths use XDG Base Directory specification:
- **Config**: `~/.config/nabi/`
- **Data**: `~/.local/share/nabi/`
- **State**: `~/.local/state/nabi/`

### Tool Registry
Promotion integrates with tool registry (`nabi tool list`).

## Best Practices

1. **Use STABLE for production**: Immutable, rollback-safe
2. **Use LIVE for development**: Fast iteration, no version overhead
3. **Semantic versioning**: Follow semver (major.minor.patch)
4. **Test before promoting**: Validate in source before promotion
5. **Document changes**: Update tool TOML with promotion metadata

## Troubleshooting

### "No [promotion] section in tool config"
Add `[promotion]` section to `~/.config/nabi/tools/{tool_id}.toml`.

### "Source not found"
Verify `source` paths exist and use `~/` prefix for XDG compliance.

### "Invalid version format"
Use semantic versioning: `1.0.0`, `2.3.1-beta`, etc. (or `live` for LIVE mode).

### Version alias not updating
Re-run promotion with same version to update aliases.

## Examples

See `~/.config/nabi/tools/cursorignore.toml` for a complete working example.
