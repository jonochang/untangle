# Total Complexity

## Formula

```
total_complexity = nodes + edges + max_depth
```

## Purpose

Total complexity is a single composite metric that captures the overall structural complexity of the dependency graph. It combines three dimensions:

| Component | What It Captures |
|-----------|-----------------|
| `nodes` | Number of modules (size) |
| `edges` | Number of dependencies (coupling) |
| `max_depth` | Longest dependency chain (layering) |

## Usage

Total complexity is most useful for **trend tracking** in the `diff` command. The `total_complexity_delta` field shows whether overall structural complexity is growing or shrinking between revisions.

```json
{
  "summary_delta": {
    "total_complexity_delta": 27
  }
}
```

A positive delta means the codebase became more structurally complex.

## Interpretation

The absolute value of total complexity depends heavily on project size. Compare it over time rather than against fixed thresholds:

- **Increasing**: More modules, more coupling, or deeper chains
- **Stable**: Structural complexity is under control
- **Decreasing**: Successful decoupling or simplification
