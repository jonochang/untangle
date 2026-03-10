# JSON Format

JSON is the default machine-readable format for most Untangle commands.

## Common v2 Envelope

Every JSON response now begins with:

```json
{
  "kind": "analyze.report",
  "schema_version": 2
}
```

The `kind` value identifies the command/view that produced the payload.

## `analyze report`

```json
{
  "kind": "analyze.report",
  "schema_version": 2,
  "metadata": {
    "language": "python",
    "granularity": "module",
    "root": "/path/to/project/src",
    "node_count": 342,
    "edge_count": 1208
  },
  "summary": {},
  "hotspots": [],
  "sccs": [],
  "insights": []
}
```

## `analyze graph`

```json
{
  "kind": "analyze.graph",
  "schema_version": 2,
  "nodes": [],
  "edges": []
}
```

## `analyze architecture`

```json
{
  "kind": "analyze.architecture",
  "schema_version": 2,
  "nodes": [],
  "edges": []
}
```

## `diff`

`diff` is wrapped under a `report` field:

```json
{
  "kind": "diff.report",
  "schema_version": 2,
  "report": {
    "base_ref": "origin/main",
    "head_ref": "HEAD",
    "verdict": "fail"
  }
}
```

## `quality`

```json
{
  "kind": "quality.functions",
  "schema_version": 2,
  "report": {
    "metadata": {
      "metric": "crap"
    },
    "results": []
  }
}
```

```json
{
  "kind": "quality.project",
  "schema_version": 2,
  "report": {
    "metadata": {
      "metric": "overall"
    },
    "overall": {}
  }
}
```

## `service-graph`

```json
{
  "kind": "service_graph",
  "schema_version": 2,
  "services": [],
  "cross_service_edges": []
}
```
