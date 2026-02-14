# Fan-out & Fan-in

## Fan-out

**Fan-out** is the number of distinct modules that a module depends on (outgoing edges in the dependency graph).

A module with high fan-out has many dependencies. This often indicates a module with too many responsibilities — it needs to know about many other parts of the system.

### Interpretation

| Fan-out | Interpretation |
|---------|---------------|
| 0 | Leaf module — depends on nothing |
| 1-5 | Normal — focused module |
| 5-10 | Moderate — worth monitoring |
| 10+ | High — consider decomposing |

## Fan-in

**Fan-in** is the number of distinct modules that depend on this module (incoming edges).

A module with high fan-in is widely used. This isn't inherently bad (utility modules naturally have high fan-in), but a module with both high fan-out AND high fan-in is a "god module" — a central hub that is hard to change without cascading effects.

### Interpretation

| Fan-in | Interpretation |
|--------|---------------|
| 0 | Entry point or unused module |
| 1-5 | Normal usage |
| 5+ | Widely depended upon — changes here have high blast radius |

## Summary Statistics

For both fan-out and fan-in, the summary includes:

- **mean** — average across all modules
- **p90** — 90th percentile (the value below which 90% of modules fall)
- **max** — the highest value in the graph

These are used by [insight rules](../insights/README.md) to set relative thresholds. For example, the high fan-out rule triggers when a module's fan-out exceeds the p90 value.

## Edge Provenance

Every edge in the graph carries **source locations** — the exact file and line number of the import statement that caused it. When a module imports another module from multiple locations, the edge has multiple source locations and a higher weight.
