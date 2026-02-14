# Circular Dependency

A circular dependency insight is generated for each non-trivial [SCC](../metrics/scc.md) (strongly connected component) in the dependency graph.

## Detection

One insight per SCC. This is a graph-level insight (module field is `(graph-level)`).

## Default Configuration

```toml
[rules.circular_dependency]
enabled = true
warning_min_size = 4      # SCC size >= 4 upgrades to warning
```

| Field | Default | Description |
|-------|---------|-------------|
| `enabled` | `true` | Enable circular dependency detection |
| `warning_min_size` | `4` | SCC size threshold for warning severity |

## Severity

| Condition | Severity |
|-----------|----------|
| SCC size >= `warning_min_size` | **Warning** |
| SCC size < `warning_min_size` | **Info** |

Small cycles (2-3 modules) get Info severity since they may be intentional. Larger cycles are more likely to be structural debt.

## Example Messages

```
[i] Modules a, b, c form a circular dependency (SCC #0, 3 modules).
    Consider introducing an interface to break this cycle.

[!] Modules w, x, y, z, ... form a circular dependency (SCC #1, 8 modules).
    Consider introducing an interface to break this cycle.
```

## Remediation

- **Dependency inversion**: Introduce an interface/trait that both modules depend on instead of depending on each other
- **Extract shared logic**: Move shared code into a third module that both depend on
- **Merge modules**: If two modules are truly inseparable, consider merging them
- **Event-based decoupling**: Replace direct calls with events or callbacks
