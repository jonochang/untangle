# Quick Start

## Analyze a Project

Run `untangle analyze` on a source directory:

```bash
untangle analyze ./src --lang python
```

This will parse all Python files, build the dependency graph, compute metrics, and print a JSON report to stdout.

For human-readable output, use `--format text`:

```bash
untangle analyze ./src --lang python --format text
```

Example output:

```
Untangle Analysis Report
========================

Language:   python
Root:       /home/user/project/src
Nodes:      342
Edges:      1208
Density:    0.0103

Summary
-------
Fan-out:  mean=3.53  p90=8  max=23
Fan-in:   mean=3.53  p90=7  max=19
SCCs:     4 (largest: 12, total nodes: 29)
Depth:    max=7  avg=4.20
Complexity: 1557 (nodes + edges + max_depth)

Top 20 Hotspots
------------------------------------------------------------
Module                                    Fan-out   Fan-in   SCC
src/core/engine.py                             23        5     -
src/api/middleware.py                          18       12    #0
src/handlers/dispatch.py                      15        3     -

Insights
------------------------------------------------------------
  [!] Module 'src/api/middleware.py' has both high fan-out (18)
      and high fan-in (12), suggesting it may be acting as a
      central hub. Consider decomposing it to reduce coupling.
  [i] Modules a, b, c form a circular dependency (SCC #0, 12
      modules). Consider introducing an interface to break
      this cycle.
```

## Diff Between Git Revisions

The core CI use case: compare structural metrics between two git refs.

```bash
untangle diff --base origin/main --head HEAD \
  --fail-on fanout-increase,new-scc
```

This exits with code `1` if any module's fan-out increased or a new circular dependency appeared.

## Export a Dependency Graph

Generate a Graphviz DOT file and render it:

```bash
untangle graph ./src --lang go --format dot | dot -Tsvg -o deps.svg
```

Or export as JSON for custom tooling:

```bash
untangle graph ./src --lang rust --format json > graph.json
```

## Inspect Configuration

See the fully resolved configuration (merging defaults, config files, env vars, and CLI flags):

```bash
untangle config show
```

Understand where a specific rule's thresholds come from:

```bash
untangle config explain high_fanout
```
