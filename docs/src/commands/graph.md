# graph

The raw graph export now lives under `analyze`:

```bash
untangle analyze graph [PATH] [OPTIONS]
```

Use this command when you want the full dependency graph rather than the default report projection.

## Formats

- `dot` for Graphviz and visual rendering
- `json` for custom tooling

## Examples

```bash
untangle analyze graph ./src --lang go --format dot | dot -Tsvg -o deps.svg
untangle analyze graph ./src --lang python --format dot | dot -Tpng -o deps.png
untangle analyze graph ./src --lang rust --format json > graph.json
untangle analyze graph ./src --lang python --format dot | dot -Tsvg -o /tmp/deps.svg && open /tmp/deps.svg
```

## JSON Output

The JSON output now starts with a v2 envelope:

```json
{
  "kind": "analyze.graph",
  "schema_version": 2,
  "nodes": [
    { "kind": "module", "path": "src/core/engine.py", "name": "src/core/engine" }
  ],
  "edges": [
    {
      "from": "src/api/handler",
      "to": "src/core/engine",
      "source_locations": [
        { "file": "src/api/handler.py", "line": 3, "column": 0 }
      ]
    }
  ]
}
```
