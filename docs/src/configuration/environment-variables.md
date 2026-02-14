# Environment Variables

Untangle reads `UNTANGLE_*` environment variables at layer 4 of the [resolution order](./resolution.md) (above config files, below CLI flags).

## Supported Variables

| Variable | Type | Description |
|----------|------|-------------|
| `UNTANGLE_FORMAT` | string | Output format (`json`, `text`, `dot`, `sarif`) |
| `UNTANGLE_LANG` | string | Language (`python`, `ruby`, `go`, `rust`) |
| `UNTANGLE_QUIET` | bool | Suppress progress (`1`, `true`, or `0`, `false`) |
| `UNTANGLE_TOP` | integer | Number of top hotspots |
| `UNTANGLE_INCLUDE_TESTS` | bool | Include test files (`1`/`true`) |
| `UNTANGLE_FAIL_ON` | comma-separated | Fail-on conditions (e.g., `fanout-increase,new-scc`) |
| `UNTANGLE_INCLUDE` | comma-separated | Include glob patterns |
| `UNTANGLE_EXCLUDE` | comma-separated | Exclude glob patterns |

## Boolean Values

Boolean variables accept `1` or `true` (case-insensitive) for true. Any other value is treated as false.

## List Values

Variables that accept lists use comma-separated values:

```bash
export UNTANGLE_FAIL_ON="fanout-increase,new-scc,scc-growth"
export UNTANGLE_EXCLUDE="vendor/**,**/test/**"
```

## Examples

### Set default format for your shell

```bash
export UNTANGLE_FORMAT=text
untangle analyze ./src  # Uses text format
```

### CI-specific overrides

```bash
UNTANGLE_QUIET=1 UNTANGLE_FORMAT=json untangle analyze ./src > results.json
```

### Override fail conditions in CI

```bash
UNTANGLE_FAIL_ON="fanout-increase,new-scc" untangle diff --base origin/main --head HEAD
```

## Precedence

Environment variables override user and project config files, but CLI flags take highest priority:

```bash
# UNTANGLE_FORMAT=text is overridden by --format json
UNTANGLE_FORMAT=text untangle analyze ./src --format json  # JSON wins
```
