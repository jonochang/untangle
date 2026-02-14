# High Fan-out

A module with high fan-out depends on many other modules, suggesting it may have too many responsibilities.

## Detection

A module triggers the high fan-out insight when:

- `relative_to_p90 = true` (default): fan-out > p90 AND fan-out >= `min_fanout`
- `relative_to_p90 = false`: fan-out >= `min_fanout`

Modules already flagged as [god modules](./god-module.md) are excluded.

## Default Configuration

```toml
[rules.high_fanout]
enabled = true
min_fanout = 5            # Minimum absolute fan-out
relative_to_p90 = true    # Also require exceeding p90
warning_multiplier = 2    # Fan-out >= 2*p90 upgrades to warning
```

| Field | Default | Description |
|-------|---------|-------------|
| `enabled` | `true` | Enable high fan-out detection |
| `min_fanout` | `5` | Minimum absolute fan-out to trigger |
| `relative_to_p90` | `true` | Also require fan-out > p90 |
| `warning_multiplier` | `2` | Multiplier of p90 that upgrades severity to warning |

## Severity

| Condition | Severity |
|-----------|----------|
| fan-out >= `warning_multiplier * p90` | **Warning** |
| Otherwise | **Info** |

For example, with default settings and p90=8:
- Fan-out of 10: Info (above p90 but below 2*p90=16)
- Fan-out of 20: Warning (above 2*p90=16)

## Example Message

```
[i] Module 'src/core/engine' has a fan-out of 12 (p90=8). Consider whether it
    has too many responsibilities and might benefit from being split.
```

## Remediation

- **Split the module**: Break it into smaller, focused modules
- **Extract common dependencies**: If many imports serve the same purpose, consolidate behind an abstraction
- **Review necessity**: Some imports may be unnecessary or could be replaced with simpler alternatives
