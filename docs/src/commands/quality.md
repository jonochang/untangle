# quality

`quality` is now split by user-facing task:

- `untangle quality functions [PATH]`
- `untangle quality report [PATH]`
- `untangle quality project [PATH]`

## `quality functions`

Compute function-level quality metrics such as CRAP.

### Usage

```bash
untangle quality functions [PATH] --metric crap --coverage lcov.info --lang rust
```

### Options

- `--metric <NAME>`: function metric to compute (`crap`)
- `--coverage <FILE>`: LCOV coverage file
- `--lang <LANG>`: limit analysis to a language (`rust`, `go`, `python`, `ruby`)
- `--format <FMT>`: `json` or `text`
- `--top <N>`: show only top N results
- `--min-cc <N>`: minimum cyclomatic complexity to include (default: 2)
- `--min-score <N>`: minimum metric score to include (default: 0)
- `--include-tests`: include test files
- `--include <GLOB>`: include glob patterns
- `--exclude <GLOB>`: exclude glob patterns
- `--quiet`: suppress progress output

### Example

```bash
cargo llvm-cov --lcov --output-path lcov.info
untangle quality functions . --metric crap --coverage lcov.info --lang rust --format text
```

## `quality project`

Compute the project-level quality summary that combines structural metrics with function-level quality data.

### Usage

```bash
untangle quality project [PATH] --coverage lcov.info --lang rust --format text
```

### Example

```bash
untangle quality project . --coverage lcov.info --lang rust --format text
```

## `quality report`

Compute the unified engineer-facing quality report. This combines:

- structural metrics, hotspots, SCCs, and insights
- function quality results (`crap` when `--coverage` is provided, otherwise `complexity`)
- a layered architecture view with feedback edges and DOT output
- a ranked list of priority actions

### Usage

```bash
untangle quality report [PATH] [OPTIONS]
```

### Options

- `--coverage <FILE>`: optional LCOV coverage file
- `--lang <LANG>`: limit analysis to a language (`rust`, `go`, `python`, `ruby`)
- `--format <FMT>`: `json` or `text`
- `--top <N>`: limit hotspots, function results, and priority actions
- `--min-cc <N>`: minimum cyclomatic complexity to include (default: 2)
- `--min-score <N>`: minimum metric score to include (default: 0)
- `--architecture-level <N>`: hierarchy depth for the embedded architecture view
- `--include-tests`: include test files
- `--include <GLOB>`: include glob patterns
- `--exclude <GLOB>`: exclude glob patterns
- `--quiet`: suppress progress output

### Examples

```bash
untangle quality report . --coverage lcov.info --lang rust --format text
untangle quality report . --lang python --format json
```
