# diff

Compare dependency graphs between two git revisions and optionally fail on policy violations.

## Usage

```bash
untangle diff [PATH] --base <REF> --head <REF> [OPTIONS]
```

## Arguments

| Argument | Description |
|----------|-------------|
| `PATH` | Path to the repository (defaults to current directory) |

## Options

| Flag | Type | Description |
|------|------|-------------|
| `--base` | git ref | Base git reference (required). E.g., `origin/main`, `HEAD~5`, a commit SHA. |
| `--head` | git ref | Head git reference (required). E.g., `HEAD`, a branch name. |
| `--lang` | `python\|ruby\|go\|rust` | Language to analyze. Auto-detected if omitted. |
| `--format` | `json\|text\|sarif` | Output format. Default: `json`. (`sarif` falls back to JSON with a warning.) |
| `--fail-on` | conditions | Comma-separated [fail-on conditions](../ci-integration/fail-on.md). |
| `--include-tests` | flag | Include test files. |
| `--include` | glob | Include glob patterns (repeatable). |
| `--exclude` | glob | Exclude glob patterns (repeatable). |
| `--quiet` | flag | Suppress progress output. |

## Examples

### Basic diff

```bash
untangle diff --base origin/main --head HEAD
```

### CI gate with fail conditions

```bash
untangle diff --base origin/main --head HEAD \
  --fail-on fanout-increase,new-scc,scc-growth
```

### Diff a specific directory

```bash
untangle diff ./src --base v1.0.0 --head v2.0.0 --lang python
```

## Output

The diff output includes:

- **verdict**: `pass` or `fail`
- **reasons**: which fail-on conditions triggered (if any)
- **summary_delta**: changes in node count, edge count, SCC count, mean fan-out, mean entropy, max depth, total complexity
- **new_edges**: edges added between base and head
- **removed_edges**: edges removed
- **fanout_changes**: modules whose fan-out changed, with before/after values and entropy
- **scc_changes**: new SCCs, enlarged SCCs, and resolved (removed) SCCs

Note: `dot` output is not supported for `diff`. `sarif` is accepted but currently falls back to JSON with a warning.

## Verdicts

| Verdict | Meaning |
|---------|---------|
| `pass` | No `--fail-on` conditions were triggered |
| `fail` | One or more conditions triggered |

## Exit Codes

| Code | Meaning |
|------|---------|
| `0` | No policy violations (verdict: pass) |
| `1` | One or more `--fail-on` conditions triggered (verdict: fail), or an error occurred |
