# Output Formats

Untangle supports four output formats, selected via `--format`:

| Format | Flag Value | Best For |
|--------|-----------|----------|
| [JSON](./json.md) | `json` | CI pipelines, custom tooling, programmatic access |
| [Text](./text.md) | `text` | Human-readable terminal output |
| [DOT](./dot.md) | `dot` | Graph visualization with Graphviz |
| [SARIF](./sarif.md) | `sarif` | GitHub Code Scanning integration |

The default format is `json`, configurable via the [config file](../configuration/config-file.md) or `UNTANGLE_FORMAT` environment variable.

## Format Availability by Command

| Command | JSON | Text | DOT | SARIF |
|---------|------|------|-----|-------|
| `analyze` | Default | Full report | Graph only | Warnings |
| `diff` | Default | Summary + changes | - | Falls back to JSON (warning) |
| `graph` | Nodes + edges | - | Graph visualization | - |
| `service-graph` | Default | Summary + edges | Graph visualization | - |
