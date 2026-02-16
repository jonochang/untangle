# untangle

A fast, multi-language dependency graph analyzer that catches structural regressions in CI.

Untangle builds module-level dependency graphs from your source code, computes structural complexity metrics, and diffs them between git revisions. Add it to your CI pipeline to fail PRs that introduce circular dependencies or increase coupling.

## Supported Languages

| Language | Granularity | Parser |
|----------|-------------|--------|
| Python | Module (file-level) | tree-sitter |
| Ruby | File-level | tree-sitter |
| Go | Package-level | tree-sitter |
| Rust | Module (file-level) | tree-sitter |

## Quick Start

```bash
# Analyze current state
untangle analyze ./src --lang python

# Diff against main (the CI use case)
untangle diff --base origin/main --head HEAD --fail-on fanout-increase,new-scc

# Export graph for visualization
untangle graph ./src --lang go --format dot | dot -Tsvg -o deps.svg
```

## What It Measures

- **Fan-out / Fan-in** — how many modules does each module depend on (and how many depend on it)?
- **Fan-out entropy** — Shannon entropy of outgoing edges. Distinguishes "depends on 10 things equally" from "depends on 10 things but 90% on one."
- **Strongly connected components** — circular dependency clusters. The hardest structural debt to unwind.
- **SCC-adjusted entropy** — penalises complexity inside circular clusters.
- **Edge provenance** — every edge carries the source module path and import line/column. (For Go, the path is the package directory.)

## CI Integration

```yaml
- name: Dependency structure check
  run: |
    untangle diff --base origin/main --head ${{ github.sha }} \
      --fail-on fanout-increase,new-scc \
      --format sarif > untangle.sarif
```

| Exit Code | Meaning |
|-----------|---------|
| `0` | No policy violations |
| `1` | One or more `--fail-on` conditions triggered, or an error occurred |

### Fail-on Conditions

| Condition | Triggers when |
|-----------|--------------|
| `fanout-increase` | Any module's fan-out increased |
| `fanout-threshold` | Any module exceeds `--threshold-fanout` |
| `new-scc` | A new circular dependency cluster appeared |
| `scc-growth` | An existing circular cluster gained members |
| `entropy-increase` | Graph-level mean entropy increased |
| `new-edge` | Any new dependency edge was added (strict mode) |

## Configuration

Create a `.untangle.toml` in your project root:

```toml
[defaults]
lang = "python"
exclude = ["vendor/**", "**/test/**", "**/*_test.go"]

[thresholds]
max_fanout = 15
max_scc_size = 5

[fail_on]
conditions = ["fanout-increase", "new-scc", "scc-growth"]
```

## Example Output

```
$ untangle analyze ./src --lang python --format text

  Modules: 342    Edges: 1,208    Density: 0.010
  Fan-out: mean 3.5 · p90 8 · max 23
  SCCs: 4 (29 modules in circular clusters, largest: 12)

  Top fan-out:
    src/core/engine.py         23  (entropy: 4.12)
    src/api/middleware.py       18  (entropy: 3.89)  ⚠ SCC #0 (12 members)
    src/handlers/dispatch.py   15  (entropy: 3.74)

  2 files skipped (syntax errors), 14 unresolved imports
  Completed in 0.85s (403 modules/sec)
```

## Installation

```bash
# From crates.io
cargo install untangle

# From source
git clone https://github.com/jonochang/untangle && cd untangle
cargo build --release
```

Pre-built binaries for Linux, macOS, and Windows are available on the [releases page](https://github.com/jonochang/untangle/releases).

## How It Works

Untangle uses [tree-sitter](https://tree-sitter.github.io/) to parse source files into concrete syntax trees, then extracts import statements via language-specific S-expression queries. This means it handles files with syntax errors gracefully (partial parse) and runs at >10,000 files/sec.

The extracted imports are resolved to project-internal modules and assembled into a directed graph using [petgraph](https://docs.rs/petgraph/). Metrics are computed using Tarjan's algorithm for SCCs and standard graph traversal for fan-out/fan-in.

For `diff`, untangle reads files at arbitrary git refs via [libgit2](https://libgit2.org/) without checking out branches, keeping the operation fast and non-destructive.

## Design Documents

- [Requirements specification](docs/specs/untangle-requirements.md) — what the tool does, output formats, failure modes
- [Technical design](docs/specs/untangle-design.md) — Rust implementation plan, data model, testing strategy

## License

MIT OR Apache-2.0
