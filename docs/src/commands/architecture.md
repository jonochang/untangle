# architecture

The architecture projection now lives under `analyze`:

```bash
untangle analyze architecture [PATH] [OPTIONS]
```

This command reuses Untangle's normal parsing and resolution pipeline, then groups modules into higher-level components based on logical module namespace, falling back to path hierarchy when needed.

## Options

| Flag | Description |
|------|-------------|
| `--lang <LANG>` | Analyze a single language (`python`, `ruby`, `go`, `rust`) |
| `--format <FMT>` | Output format: `json` or `dot` |
| `--level <N>` | Project to hierarchy depth `N` |
| `--include-tests` | Include test files |
| `--include <GLOB>` | Include matching files |
| `--exclude <GLOB>` | Exclude matching files |
| `--quiet` | Suppress progress output |

## Examples

```bash
untangle analyze architecture ./src --lang python --format json
untangle analyze architecture ./src --lang ruby --level 2 --format json
untangle analyze architecture ./src --lang go --format dot | dot -Tsvg -o architecture.svg
```

## JSON Output

The JSON output now starts with a v2 envelope:

```json
{
  "kind": "analyze.architecture",
  "schema_version": 2,
  "nodes": [
    {
      "id": "api",
      "label": "api",
      "layer": 0,
      "module_count": 4
    }
  ],
  "edges": [
    {
      "from": "api",
      "to": "db",
      "count": 3,
      "source_location_count": 6,
      "feedback": false
    }
  ]
}
```
