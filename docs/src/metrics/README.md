# Metrics

Untangle computes structural metrics on the dependency graph to quantify coupling, complexity, and modularity.

## Metrics at a Glance

| Metric | Level | Description |
|--------|-------|-------------|
| [Fan-out](./fanout-fanin.md) | Per-module | Number of modules this module depends on |
| [Fan-in](./fanout-fanin.md) | Per-module | Number of modules that depend on this module |
| [Entropy](./entropy.md) | Per-module | Shannon entropy of outgoing edge weights |
| [SCC-adjusted entropy](./entropy.md) | Per-module | Entropy amplified by circular dependency membership |
| [SCC count](./scc.md) | Graph-level | Number of non-trivial strongly connected components |
| [SCC size](./scc.md) | Per-SCC | Number of modules in a circular dependency cluster |
| [Max depth](./depth.md) | Graph-level | Longest dependency chain in the condensation DAG |
| [Avg depth](./depth.md) | Graph-level | Average chain length across all root-to-leaf paths |
| [Total complexity](./total-complexity.md) | Graph-level | Composite metric: nodes + edges + max_depth |

## Summary Statistics

The `summary` object in JSON output includes aggregate statistics:

```json
{
  "mean_fanout": 3.53,
  "p90_fanout": 8,
  "max_fanout": 23,
  "mean_fanin": 3.53,
  "p90_fanin": 7,
  "max_fanin": 19,
  "scc_count": 4,
  "largest_scc_size": 12,
  "total_nodes_in_sccs": 29,
  "max_depth": 7,
  "avg_depth": 4.2,
  "total_complexity": 1557
}
```

- `mean` — arithmetic mean across all modules
- `p90` — 90th percentile value
- `max` — maximum value across all modules
