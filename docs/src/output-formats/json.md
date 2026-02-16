# JSON Format

JSON is the default output format, designed for programmatic consumption in CI pipelines and custom tooling.

## Analyze Output Schema

```json
{
  "metadata": {
    "language": "python",
    "granularity": "module",
    "root": "/path/to/project/src",
    "node_count": 342,
    "edge_count": 1208,
    "edge_density": 0.0103,
    "files_parsed": 340,
    "files_skipped": 2,
    "unresolved_imports": 14,
    "timestamp": "...",
    "elapsed_ms": 850,
    "modules_per_second": 402.4,
    "languages": [
      {
        "language": "python",
        "files_parsed": 340,
        "nodes": 342,
        "imports_resolved": 1194,
        "imports_unresolved": 14
      }
    ]
  },
  "summary": {
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
  },
  "hotspots": [
    {
      "node": "src/core/engine",
      "fanout": 23,
      "fanin": 5,
      "entropy": 4.12,
      "scc_id": null,
      "scc_adjusted_entropy": 4.12,
      "fanout_edges": [
        {
          "to": "src/core/config",
          "source_locations": [
            { "file": "src/core/engine.py", "line": 3, "column": 0 }
          ]
        }
      ]
    }
  ],
  "sccs": [
    {
      "id": 0,
      "size": 12,
      "internal_edges": 18,
      "members": ["src/api/middleware", "src/api/auth", "..."]
    }
  ],
  "insights": [
    {
      "category": "god_module",
      "severity": "warning",
      "module": "src/api/middleware",
      "message": "Module 'src/api/middleware' has both high fan-out (18) and high fan-in (12)...",
      "metrics": {
        "fanout": 18,
        "fanin": 12
      }
    }
  ]
}
```

## Diff Output Schema

```json
{
  "base_ref": "origin/main",
  "head_ref": "HEAD",
  "verdict": "fail",
  "reasons": ["fanout-increase", "new-scc"],
  "elapsed_ms": 1200,
  "modules_per_second": 580.3,
  "summary_delta": {
    "nodes_added": 5,
    "nodes_removed": 1,
    "edges_added": 12,
    "edges_removed": 3,
    "net_edge_change": 9,
    "scc_count_delta": 1,
    "largest_scc_size_delta": 3,
    "mean_fanout_delta": 0.15,
    "mean_entropy_delta": 0.08,
    "max_depth_delta": 1,
    "total_complexity_delta": 27
  },
  "new_edges": [
    {
      "from": "src/api/handler",
      "to": "src/core/engine",
      "source_locations": [
        { "file": "src/api/handler.py", "line": 5, "column": 0 }
      ]
    }
  ],
  "removed_edges": [],
  "fanout_changes": [
    {
      "node": "src/api/handler",
      "fanout_before": 3,
      "fanout_after": 5,
      "delta": 2,
      "entropy_before": 1.58,
      "entropy_after": 2.32,
      "new_targets": []
    }
  ],
  "scc_changes": {
    "new_sccs": [
      { "members": ["src/a", "src/b", "src/c"], "size": 3 }
    ],
    "enlarged_sccs": [],
    "resolved_sccs": []
  }
}
```

## Service-Graph Output Schema

```json
{
  "services": [
    {
      "name": "billing",
      "root": "services/billing",
      "language": "python",
      "file_count": 412
    }
  ],
  "cross_service_edges": [
    {
      "from_service": "web",
      "to_service": "billing",
      "kind": "rest_call",
      "operation": "POST /v1/invoices",
      "source_locations": [
        { "file": "services/web/src/api/client.py", "line": 42, "column": 0 }
      ]
    }
  ]
}
```

## Key Fields

### metadata

| Field | Description |
|-------|-------------|
| `language` | Language that was analyzed (`multi` in multi-language mode) |
| `languages` | Per-language stats (present in multi-language mode) |
| `granularity` | Always `"module"` |
| `node_count` | Number of modules in the graph |
| `edge_count` | Number of dependency edges |
| `edge_density` | `edges / (nodes * (nodes - 1))` |
| `files_parsed` | Number of files successfully parsed |
| `files_skipped` | Number of files that could not be read |
| `unresolved_imports` | External, dynamic, or unresolvable imports |
| `timestamp` | UTC timestamp when analysis completed |
| `elapsed_ms` | Wall-clock time in milliseconds |
| `modules_per_second` | Processing throughput |

### hotspots

Sorted by fan-out descending (then fan-in). Use `--top N` to limit.

### insights

Only present when `--no-insights` is not set. See [Insights](../insights/README.md).
