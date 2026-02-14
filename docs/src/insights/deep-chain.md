# Deep Chain

A deep chain insight triggers when the dependency graph has an unusually long dependency chain, measured on the [condensation DAG](../metrics/depth.md).

## Detection

The insight triggers when **either** condition is met:

1. **Absolute**: `max_depth >= absolute_depth`
2. **Relative**: `max_depth > relative_multiplier * avg_depth` AND `max_depth >= relative_min_depth`

This is a graph-level insight (module field is `(graph-level)`).

## Default Configuration

```toml
[rules.deep_chain]
enabled = true
absolute_depth = 8          # Always trigger at depth >= 8
relative_multiplier = 2.0   # Trigger if depth > 2x average
relative_min_depth = 5      # Minimum depth for relative trigger
```

| Field | Default | Description |
|-------|---------|-------------|
| `enabled` | `true` | Enable deep chain detection |
| `absolute_depth` | `8` | Depth that always triggers the insight |
| `relative_multiplier` | `2.0` | How many times average triggers relative check |
| `relative_min_depth` | `5` | Minimum depth for the relative check |

## Severity

Always **Info** (`[i]`) — deep chains are a structural signal, not necessarily a problem.

## Example Message

```
[i] The longest dependency chain is 9 levels deep (avg: 4.2). Deep chains may
    increase build times. Consider consolidating intermediate modules.
```

## Why Two Thresholds?

The **absolute** threshold catches objectively deep chains regardless of project size.

The **relative** threshold catches chains that are unusually deep compared to the project's average. A project with avg_depth=3 and max_depth=7 has an outlier chain, even though 7 is below the absolute threshold of 8.

## Remediation

- **Flatten layers**: Merge intermediate modules that only pass through calls
- **Reduce indirection**: Remove unnecessary abstraction layers
- **Direct dependencies**: Consider depending on lower-level modules directly if the intermediate layers don't add value
