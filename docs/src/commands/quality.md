# quality

Compute function-level code-quality metrics (starting with CRAP).

## Usage

```bash
untangle quality <PATH> --metric crap --coverage lcov.info --lang rust
```

## Arguments

- `<PATH>`: root directory to analyze

## Options

- `--metric <NAME>`: metric to compute (`crap`, `overall`)
- `--coverage <FILE>`: LCOV coverage file (required for CRAP and overall)
- `--lang <LANG>`: limit analysis to a language (`rust`, `go`, `python`, `ruby`)
- `--format <FMT>`: `json` or `text` (default: config format)
- `--top <N>`: show only top N results
- `--min-cc <N>`: minimum cyclomatic complexity to include (default: 2)
- `--min-score <N>`: minimum metric score to include (default: 0)
- `--include-tests`: include test files
- `--include <GLOB>`: include glob patterns
- `--exclude <GLOB>`: exclude glob patterns
- `--quiet`: suppress progress output

## Examples

```bash
# Rust project with cargo-llvm-cov
cargo llvm-cov --lcov --output-path lcov.info
untangle quality . --metric crap --coverage lcov.info --lang rust --format text
```

```bash
# Overall report (Untangle + CRAP)
untangle quality . --metric overall --coverage lcov.info --lang rust --format text
```
