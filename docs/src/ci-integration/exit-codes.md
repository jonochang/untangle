# Exit Codes

Untangle uses three exit codes:

| Exit Code | Meaning |
|-----------|---------|
| `0` | Success — no policy violations |
| `1` | Policy violation — one or more `--fail-on` conditions triggered |
| `2` | Error — analysis could not complete |

## When Each Code Is Used

### Exit Code 0

- `analyze`: Analysis completed successfully
- `diff`: No `--fail-on` conditions triggered (verdict: `pass`)
- `graph`: Graph exported successfully
- `config`: Configuration displayed successfully

### Exit Code 1

- `diff` only: One or more `--fail-on` conditions were triggered (verdict: `fail`)

This is the primary CI gate mechanism. The `reasons` field in the JSON output lists which conditions triggered.

### Exit Code 2

- Any command: Fatal error preventing analysis
  - No source files found at the given path
  - Invalid git refs for `diff`
  - Unreadable config file
  - Invalid path

## CI Usage

```yaml
# Simple: fail the CI step if any condition triggers
- name: Structural check
  run: untangle diff --base origin/main --head HEAD --fail-on new-scc

# Advanced: capture output and handle exit codes
- name: Structural check
  run: |
    untangle diff --base origin/main --head HEAD \
      --fail-on fanout-increase,new-scc \
      --format json > diff-result.json
  continue-on-error: true

- name: Upload results
  if: always()
  uses: actions/upload-artifact@v4
  with:
    name: untangle-diff
    path: diff-result.json
```
