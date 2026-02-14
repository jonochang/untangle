# GitHub Actions

## Complete Workflow Example

```yaml
name: Dependency Structure Check

on:
  pull_request:

jobs:
  untangle:
    name: Structural Analysis
    runs-on: ubuntu-latest
    steps:
      - uses: actions/checkout@v4
        with:
          fetch-depth: 0  # Full history needed for diff

      - name: Install untangle
        run: cargo install untangle

      - name: Run structural diff
        run: |
          untangle diff \
            --base origin/${{ github.base_ref }} \
            --head ${{ github.sha }} \
            --fail-on fanout-increase,new-scc \
            --format json \
            --quiet \
            > diff-result.json

      - name: Upload results
        if: always()
        uses: actions/upload-artifact@v4
        with:
          name: untangle-diff
          path: diff-result.json
```

## With Nix (matching this project's CI)

If your project uses Nix:

```yaml
name: Dependency Structure Check

on:
  pull_request:

jobs:
  untangle:
    name: Structural Analysis
    runs-on: ubuntu-latest
    steps:
      - uses: actions/checkout@v4
        with:
          fetch-depth: 0
      - uses: DeterminateSystems/nix-installer-action@main
      - uses: DeterminateSystems/magic-nix-cache-action@main

      - name: Build untangle
        run: nix develop --command cargo build --release

      - name: Run structural diff
        run: |
          nix develop --command ./target/release/untangle diff \
            --base origin/${{ github.base_ref }} \
            --head ${{ github.sha }} \
            --fail-on fanout-increase,new-scc \
            --format json \
            --quiet \
            > diff-result.json

      - name: Upload results
        if: always()
        uses: actions/upload-artifact@v4
        with:
          name: untangle-diff
          path: diff-result.json
```

## With SARIF Upload

See [SARIF Upload](./sarif-upload.md) for adding GitHub Code Scanning results.

## Tips

- **`fetch-depth: 0`** is required for `diff` to access both base and head git refs
- **`--quiet`** suppresses progress bars that clutter CI logs
- **`if: always()`** on the upload step ensures results are saved even on failure
- Use `continue-on-error: true` if you want to upload results but not fail the job
- Consider caching the cargo build to speed up subsequent runs
