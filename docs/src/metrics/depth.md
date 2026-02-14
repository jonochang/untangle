# Depth

## Definition

**Depth** measures the length of dependency chains in the graph. It is computed on the **condensation DAG** — the graph with SCCs collapsed to single nodes — ensuring it's well-defined even for cyclic graphs.

## Metrics

| Metric | Description |
|--------|-------------|
| `max_depth` | Longest path from any root to any leaf in the condensation DAG |
| `avg_depth` | Average longest path across root-to-leaf paths |

## Why Depth Matters

Deep dependency chains have practical consequences:

- **Build time**: In build systems with incremental compilation, deeper chains mean more sequential build steps
- **Change propagation**: A change at the bottom of a deep chain can ripple through many layers
- **Comprehension**: Understanding a module may require understanding its entire chain of transitive dependencies

## Interpretation

| Max Depth | Interpretation |
|-----------|---------------|
| 0-3 | Shallow — good modularity |
| 4-7 | Moderate — typical for medium projects |
| 8+ | Deep — may indicate over-layering |

## Condensation DAG

The condensation collapses each SCC into a single node, producing a DAG (directed acyclic graph). This is necessary because cycles would make depth undefined.

For example, if modules A, B, C form a cycle and D depends on A:

```
[A, B, C] → D
```

The condensation treats the SCC as a single node, so the depth is 1.

## Configuration

The [deep chain insight](../insights/deep-chain.md) triggers based on depth thresholds:

```toml
[rules.deep_chain]
absolute_depth = 8       # Always trigger at this depth
relative_multiplier = 2.0 # Trigger if depth > 2x average
relative_min_depth = 5    # Min depth for relative trigger
```
