# quality

`quality` is now split by user-facing task:

- `untangle quality functions [PATH]`
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
