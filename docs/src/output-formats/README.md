# Output Formats

Untangle uses per-command format enums in v2, so the available `--format` values depend on the command you run.

| Command | JSON | Text | DOT | SARIF |
|---------|------|------|-----|-------|
| `analyze report` | Full structural report | Full report | - | Warnings for code scanning |
| `analyze graph` | Raw nodes and edges | - | Graph visualization | - |
| `analyze architecture` | Layered projection | - | Layered graph visualization | - |
| `diff` | Diff report | Summary + changes | - | - |
| `quality functions` | Function-quality report | Full report | - | - |
| `quality report` | Unified quality report | Unified quality report | - | - |
| `quality project` | Project-quality report | Full report | - | - |
| `service-graph` | Service dependency report | Summary + edges | Graph visualization | - |

JSON outputs in v2 all begin with:

```json
{
  "kind": "analyze.report",
  "schema_version": 2
}
```

See the per-format pages for examples.
