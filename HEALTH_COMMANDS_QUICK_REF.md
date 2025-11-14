# Health Namespace Quick Reference

## New Commands (Use These)

```bash
# Bootstrap validation (replaces 'nabi doctor')
nabi health quick

# Substrate validation (replaces 'nabi health check')
nabi health substrate [--auto-remediate] [--fsm-only]

# Federation services (replaces 'nabi federation health')
nabi health services

# Port validation (also available as 'nabi port check')
nabi health ports

# Existing commands (unchanged)
nabi health status [--detailed] [--hours 24]
nabi health report [--format text|json|markdown] [--output FILE]
nabi health dashboard [--port 3000]
```

## Migration Cheat Sheet

| Old | New |
|-----|-----|
| `nabi doctor` | `nabi health quick` |
| `nabi health check` | `nabi health substrate` |
| `nabi federation health` | `nabi health services` |

## What Each Command Does

**quick**: Checks commander binaries + XDG compliance
**substrate**: Validates hooks, schemas, transforms
**services**: Monitors 17 federation services (Docker + LaunchAgents)
**ports**: Checks port allocations and conflicts
**status**: Historical health check results
**report**: Generate formatted health reports
**dashboard**: Open Grafana live dashboard

## Testing

```bash
# Verify binary location
ls -lh ~/.cache/nabi/nabi-cli/target/release/nabi

# See all health subcommands
~/.cache/nabi/nabi-cli/target/release/nabi health --help
```

## Reference Files

- Migration Guide: `/Users/tryk/nabia/core/nabi-cli/HEALTH_NAMESPACE_MIGRATION_GUIDE.md`
- Service Registry: `~/.config/nabi/federation-registry.toml`
- Health Config: `~/.config/nabi/health/health-checks.toml`
