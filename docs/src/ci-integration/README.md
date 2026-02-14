# CI Integration

Untangle is designed to run in CI pipelines, gating PRs on structural health of the dependency graph.

## Overview

The typical CI workflow:

1. Run `untangle diff` comparing the PR branch against the base branch
2. Use `--fail-on` to specify which structural regressions should block the PR
3. The command exits with code `1` if any conditions are triggered
4. Optionally upload SARIF results to GitHub Code Scanning

## Quick Example

```yaml
- name: Dependency structure check
  run: |
    untangle diff --base origin/main --head ${{ github.sha }} \
      --fail-on fanout-increase,new-scc \
      --format json
```

## Topics

- [Exit Codes](./exit-codes.md) — what each exit code means
- [Fail-on Conditions](./fail-on.md) — the 6 available policy checks
- [GitHub Actions](./github-actions.md) — complete workflow example
- [SARIF Upload](./sarif-upload.md) — GitHub Code Scanning integration
