# analyze

Analyze a source directory, build the dependency graph, compute metrics, and report results.

## Usage

```bash
untangle analyze <PATH> [OPTIONS]
```

## Arguments

| Argument | Description |
|----------|-------------|
| `PATH` | Path to the source directory to analyze (required) |

## Options

| Flag | Type | Description |
|------|------|-------------|
| `--lang` | `python\|ruby\|go\|rust` | Language to analyze. Auto-detected if omitted. |
| `--format` | `json\|text\|dot\|sarif` | Output format. Default: `json` (configurable). |
| `--top` | integer | Number of top hotspots to report. |
| `--threshold-fanout` | integer | Fan-out threshold for reporting / SARIF warnings. |
| `--threshold-scc` | integer | SCC size threshold for warnings. |
| `--include-tests` | flag | Include test files (e.g., Go `*_test.go`). |
| `--include` | glob | Include glob patterns (repeatable). |
| `--exclude` | glob | Exclude glob patterns (repeatable). |
| `--quiet` | flag | Suppress progress output on stderr. |
| `--no-insights` | flag | Suppress insights from output. |

## Examples

### Basic analysis

```bash
untangle analyze ./src --lang python
```

### Human-readable output with top 10 hotspots

```bash
untangle analyze ./src --lang go --format text --top 10
```

### Exclude vendor directories

```bash
untangle analyze ./src --lang go --exclude "vendor/**" --exclude "**/testdata/**"
```

### SARIF output for code scanning

```bash
untangle analyze ./src --lang python --format sarif --threshold-fanout 15 > results.sarif
```

### Quiet mode for CI

```bash
untangle analyze ./src --lang rust --format json --quiet > analysis.json
```

## Output

The command writes structured output to stdout. See [Output Formats](../output-formats/README.md) for details on each format.

Progress information is written to stderr (suppress with `--quiet`).

## Exit Codes

| Code | Meaning |
|------|---------|
| `0` | Analysis completed successfully |
| `2` | Analysis could not complete (no files found, invalid path, etc.) |
