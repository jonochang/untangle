# Text Format

The text format (`--format text`) produces a human-readable report suitable for terminal output.

## Sections

The text report includes the following sections:

### Header

```
Untangle Analysis Report
========================

Language:   python
Root:       /path/to/project/src
Nodes:      342
Edges:      1208
Density:    0.0103
Parsed:     340 files
Skipped:    2 files
Unresolved: 14 imports
```

### Summary

```
Summary
-------
Fan-out:  mean=3.53  p90=8  max=23
Fan-in:   mean=3.53  p90=7  max=19
SCCs:     4 (largest: 12, total nodes: 29)
Depth:    max=7  avg=4.20
Complexity: 1557 (nodes + edges + max_depth)
```

### Hotspots Table

```
Top 20 Hotspots
------------------------------------------------------------
Module                                    Fan-out   Fan-in   SCC
src/core/engine.py                             23        5     -
src/api/middleware.py                          18       12    #0
src/handlers/dispatch.py                      15        3     -
```

Modules are sorted by fan-out descending. SCC membership is shown as `#<id>` or `-` if not in an SCC.

### Strongly Connected Components

```
Strongly Connected Components
------------------------------------------------------------
SCC #0 (size=12, internal_edges=18)
  - src/api/middleware
  - src/api/auth
  - ...
```

### Insights

```
Insights
------------------------------------------------------------
  [!] Module 'src/api/middleware' has both high fan-out (18)
      and high fan-in (12)...
  [i] Modules a, b, c form a circular dependency...
```

Severity markers:
- `[!]` — Warning
- `[i]` — Info

### Footer

```
Completed in 0.85s (402 modules/sec)
```

## Usage

```bash
untangle analyze ./src --lang python --format text
untangle analyze ./src --lang python --format text --top 10
```
