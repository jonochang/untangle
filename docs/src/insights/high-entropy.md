# High Entropy

A high entropy insight indicates a module whose dependencies are spread broadly and evenly across many targets.

## Detection

A module triggers the high entropy insight when:

- Shannon entropy > `min_entropy` AND fan-out >= `min_fanout`
- Module is not already flagged as a [god module](./god-module.md)

## Default Configuration

```toml
[rules.high_entropy]
enabled = true
min_entropy = 2.5     # Minimum Shannon entropy
min_fanout = 5        # Minimum fan-out to consider
```

| Field | Default | Description |
|-------|---------|-------------|
| `enabled` | `true` | Enable high entropy detection |
| `min_entropy` | `2.5` | Minimum Shannon entropy threshold |
| `min_fanout` | `5` | Minimum fan-out (entropy is meaningless with few edges) |

## Severity

Always **Info** (`[i]`) — high entropy is a structural signal for potential improvement, not necessarily a defect.

## Example Message

```
[i] Module 'src/core/engine' has high dependency entropy (3.12), meaning its 10
    dependencies are spread broadly. Consider consolidating behind a facade.
```

## Interpretation

High entropy means a module's dependencies are evenly distributed. This can indicate:

- **Coordinator module**: The module orchestrates many subsystems (legitimate but worth noting)
- **Missing abstraction**: The module reaches into many unrelated areas and would benefit from a facade
- **Feature creep**: The module has accumulated responsibilities over time

Contrast with a module that has high fan-out but low entropy — it depends on many things but concentrates on one primary dependency. That pattern is often less concerning.

## Entropy Reference Values

| Dependencies | Entropy (if equal weights) |
|-------------|---------------------------|
| 2 | 1.0 |
| 4 | 2.0 |
| 6 | 2.58 |
| 8 | 3.0 |
| 16 | 4.0 |

The default `min_entropy = 2.5` corresponds roughly to 6+ equally-weighted dependencies.
