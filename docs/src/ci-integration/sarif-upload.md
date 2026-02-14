# SARIF Upload

Untangle can output SARIF 2.1.0 results for integration with GitHub Code Scanning. This surfaces structural issues as annotations directly on pull requests.

## GitHub Code Scanning Workflow

```yaml
name: Structural Analysis

on:
  pull_request:

permissions:
  security-events: write
  contents: read

jobs:
  untangle:
    name: Structural Analysis
    runs-on: ubuntu-latest
    steps:
      - uses: actions/checkout@v4

      - name: Install untangle
        run: cargo install untangle

      - name: Run analysis
        run: |
          untangle analyze . \
            --lang python \
            --format sarif \
            --threshold-fanout 15 \
            --quiet \
            > untangle.sarif

      - name: Upload SARIF
        uses: github/codeql-action/upload-sarif@v3
        with:
          sarif_file: untangle.sarif
          category: untangle
```

## What Appears in Code Scanning

Two types of findings are reported:

### High Fan-out

Modules whose fan-out exceeds the threshold (default: 10, configurable via `--threshold-fanout`) appear as warnings:

> Module 'src/core/engine' has fan-out of 23 (threshold: 15)

### Circular Dependency

Every module in a non-trivial SCC gets a warning:

> Module 'src/api/auth' is part of a circular dependency (SCC #0, 12 members)

## Configuration

Control the SARIF threshold:

```bash
# Only report modules with fan-out >= 20
untangle analyze . --format sarif --threshold-fanout 20

# Default threshold is 10
untangle analyze . --format sarif
```

## Notes

- SARIF output is supported for the `analyze` command
- The `diff` command outputs JSON (not SARIF) since diff results don't map cleanly to file-level annotations
- GitHub Code Scanning requires the `security-events: write` permission
- The `category: untangle` field ensures results are grouped separately from other SARIF tools
