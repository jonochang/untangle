# analyze

`analyze` is now an explicit command family:

- `untangle analyze report [PATH]`
- `untangle analyze graph [PATH]`
- `untangle analyze architecture [PATH]`

All three share the same parsing and dependency-resolution pipeline. They differ only in the projection they emit.

## `analyze report`

Build the dependency graph, compute metrics, and emit the default structural report.

### Usage

```bash
untangle analyze report [PATH] [OPTIONS]
```

### Options

| Flag | Type | Description |
|------|------|-------------|
| `--lang` | `python\|ruby\|go\|rust` | Language to analyze. Auto-detected if omitted. |
| `--format` | `json\|text\|sarif` | Output format. Default: `json` (configurable). |
| `--top` | integer | Number of top hotspots to report. |
| `--threshold-fanout` | integer | Fan-out threshold for reporting / SARIF warnings. |
| `--threshold-scc` | integer | SCC size threshold for warnings. |
| `--insights` | `auto\|on\|off` | Insight rendering mode. |
| `--include-tests` | flag | Include test files (e.g. Go `*_test.go`). |
| `--include` | glob | Include glob patterns (repeatable). |
| `--exclude` | glob | Exclude glob patterns (repeatable). |
| `--quiet` | flag | Suppress progress output on stderr. |

### Examples

```bash
untangle analyze report ./src --lang python
untangle analyze report ./src --lang go --format text --top 10
untangle analyze report ./src --lang python --format sarif --threshold-fanout 15 > results.sarif
```

## `analyze graph`

Export the raw dependency graph as DOT or JSON.

### Usage

```bash
untangle analyze graph [PATH] [OPTIONS]
```

### Formats

- `dot`
- `json`

### Examples

```bash
untangle analyze graph ./src --lang go --format dot | dot -Tsvg -o deps.svg
untangle analyze graph ./src --lang rust --format json > graph.json
```

## `analyze architecture`

Project the dependency graph into a layered architecture view.

### Usage

```bash
untangle analyze architecture [PATH] [OPTIONS]
```

### Options

| Flag | Description |
|------|-------------|
| `--lang <LANG>` | Analyze a single language (`python`, `ruby`, `go`, `rust`) |
| `--format <FMT>` | Output format: `json` or `dot` |
| `--level <N>` | Project to hierarchy depth `N` |
| `--include-tests` | Include test files |
| `--include <GLOB>` | Include matching files |
| `--exclude <GLOB>` | Exclude matching files |
| `--quiet` | Suppress progress output |

### Examples

```bash
untangle analyze architecture ./src --lang python --format json
untangle analyze architecture ./src --lang ruby --level 2 --format json
untangle analyze architecture ./src --lang go --format dot | dot -Tsvg -o architecture.svg
```
