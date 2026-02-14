# God Module

A **god module** is one that has both high fan-out (depends on many modules) AND high fan-in (many modules depend on it). This combination indicates a central hub that is tightly coupled with the rest of the system.

## Detection

A module triggers the god module insight when:

1. Its fan-out exceeds the threshold (AND exceeds p90 if `relative_to_p90 = true`)
2. Its fan-in exceeds the threshold (AND exceeds p90 if `relative_to_p90 = true`)

Both conditions must be met simultaneously.

## Default Configuration

```toml
[rules.god_module]
enabled = true
min_fanout = 3        # Minimum fan-out threshold
min_fanin = 3         # Minimum fan-in threshold
relative_to_p90 = true # Also require > p90 for both metrics
```

| Field | Default | Description |
|-------|---------|-------------|
| `enabled` | `true` | Enable god module detection |
| `min_fanout` | `3` | Minimum absolute fan-out |
| `min_fanin` | `3` | Minimum absolute fan-in |
| `relative_to_p90` | `true` | Also require exceeding p90 percentile |

## Severity

Always **Warning** (`[!]`) — god modules are consistently problematic.

## Example Message

```
[!] Module 'src/api/middleware' has both high fan-out (18) and high fan-in (12),
    suggesting it may be acting as a central hub. Consider decomposing it to
    reduce coupling.
```

## Relationship to Other Insights

When a module qualifies as a god module, it is **excluded** from High Fan-out and High Entropy insights to reduce noise. The god module insight subsumes both.

## Remediation

- **Split responsibilities**: Break the module into focused sub-modules
- **Introduce interfaces**: Use abstraction layers to reduce direct coupling
- **Facade pattern**: If the module is a legitimate coordination point, make it a thin facade that delegates to specialized modules
