# Insights

Untangle generates actionable insights based on the dependency graph metrics. Insights use suggestive language ("consider", "may", "might") rather than definitive statements, since structural patterns are context-dependent.

## Severity Levels

| Severity | Marker (text) | Meaning |
|----------|--------------|---------|
| **Warning** | `[!]` | Likely problematic — should be investigated |
| **Info** | `[i]` | Worth noting — may or may not need action |

## Insight Categories

| Category | Level | Triggers When |
|----------|-------|---------------|
| [God Module](./god-module.md) | Per-module | High fan-out AND high fan-in |
| [High Fan-out](./high-fanout.md) | Per-module | Fan-out above threshold |
| [Circular Dependency](./circular-dependency.md) | Graph-level | Non-trivial SCC detected |
| [Deep Chain](./deep-chain.md) | Graph-level | Dependency chain exceeds depth threshold |
| [High Entropy](./high-entropy.md) | Per-module | High entropy with high fan-out |

## Priority Order

When multiple insights apply to the same module:

1. **God Module** takes priority — if a module qualifies as a god module, it won't also generate High Fan-out or High Entropy insights (to avoid noise)
2. Insights are sorted: Warnings first, then by category, then alphabetically by module

## Disabling Insights

```bash
# CLI flag
untangle analyze ./src --no-insights

# Config file
[defaults]
no_insights = true
```

## Configuring Rules

Each insight category has configurable thresholds. See [Configuration](../configuration/config-file.md) for the full schema, or the individual insight pages for rule-specific options.

## Per-Path Overrides

Different parts of the codebase can have different thresholds. See [Per-Path Overrides](../configuration/overrides.md).
