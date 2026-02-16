# Untangle — Requirements

## Purpose

A Rust CLI tool that builds module-level dependency graphs from source code, computes structural complexity metrics (fan-out, fan-in, SCC analysis, entropy), and reports deltas between two revisions (typically base branch vs PR branch) for use in CI pipelines.

The core question it answers: **"Did this change make the dependency structure worse?"**

---

## Supported Languages

| Language | Import Mechanism | Granularity |
|----------|-----------------|-------------|
| **Python** | `import x`, `from x import y`, relative imports | Module-level (file → file) |
| **Ruby** | `require`, `require_relative`, `autoload`, Zeitwerk conventions | File-level |
| **Go** | `import "path/pkg"` | Package-level (directory → directory) |
| **Rust** | `use crate::...` | Module-level (file → file) |

Each language requires a dedicated parser frontend that produces a common intermediate graph representation.

---

## Core Concepts

### Dependency Graph

A directed graph `G = (V, E)` where:

- **V** = set of modules/packages/files (language-dependent granularity)
- **E** = set of directed edges `(a, b)` meaning "a imports/depends on b", annotated with **source locations** (file path + line numbers) that caused the edge

### Granularity Strategy

**v1: Module-level only.** Nodes represent modules (Python), files (Ruby), or packages (Go). This is the granularity where CI gates are actionable — a reviewer can look at a module-level diff and immediately understand the structural change.

**Edge provenance via line numbers.** Every edge carries the source locations (file:line) of the import statements that created it. This gives reviewers enough to pinpoint the coupling point without requiring full function-level analysis. In `diff` output, new edges report exactly which lines introduced them.

**Future: Function-level granularity.** The IR is designed to support a `--granularity function` flag in a future version. The node model uses a `kind` discriminator:

```rust
enum NodeKind {
    Module,   // v1
    Function, // future
}

struct GraphNode {
    id: NodeId,
    kind: NodeKind,
    path: PathBuf,
    name: String,
    span: Option<(usize, usize)>, // start_line, end_line — populated for Function nodes
}

struct GraphEdge {
    source_locations: Vec<SourceLocation>, // file:line of each import that created this edge
}

struct SourceLocation {
    file: PathBuf,
    line: usize,
    column: Option<usize>,
}
```

Function-level analysis answers a different question ("which function is the actual coupling point") and is better suited to interactive refactoring investigation than CI gating. It requires scope analysis within each file to resolve which imports are referenced inside which function body — significant per-language work that is out of scope for v1.

**Why not v1:** Graph size explodes (~50x more nodes), cross-language consistency becomes much harder (Go functions, Python methods-inside-classes, Ruby reopened classes/mixins), and the diffs become too noisy to act on in PR review.

### Metrics

| Metric | Definition | Why It Matters |
|--------|-----------|----------------|
| **Fan-out(v)** | Out-degree of node v | High fan-out = module knows too much, fragile to changes |
| **Fan-in(v)** | In-degree of node v | High fan-in = heavily depended upon, high blast radius |
| **Fan-out entropy H(v)** | Shannon entropy of normalised outgoing edge distribution: `H(v) = -Σ p_i log₂(p_i)` where `p_i = w_i / Σw` | Distinguishes "depends on 10 things equally" from "depends on 10 things but 90% on one" |
| **SCC set** | Strongly connected components with `|SCC| > 1` | Circular dependency clusters — the hardest structural debt to unwind |
| **SCC-adjusted entropy** | `H_struct(v) × (1 + log(|SCC_v|))` for nodes in non-trivial SCCs | Penalises complexity inside circular clusters |
| **Graph-level summary** | Total nodes, total edges, edge density, number of non-trivial SCCs, largest SCC size, mean/p90/max fan-out | Headline numbers for dashboards |

---

## CLI Interface

### Commands

```
untangle analyze [OPTIONS] <PATH>
untangle diff [OPTIONS] --base <REF> --head <REF> [PATH]
untangle graph [OPTIONS] <PATH>
untangle config <show|explain> [--path <DIR>]
untangle service-graph <PATH> [--format json|text|dot]
```

### `analyze` — Single-snapshot analysis

```
untangle analyze ./src \
  --lang python \
  --format json \
  --top 20 \
  --threshold-fanout 10 \
  --threshold-scc 3
```

**Outputs:** Full metric report for the current state of the codebase.

### `diff` — Two-revision delta (the CI-critical command)

```
untangle diff \
  --base origin/main \
  --head HEAD \
  --lang go \
  --fail-on fanout-increase,new-scc \
  --format json
```

**Outputs:** Delta report showing what changed between two revisions.

### `graph` — Export raw graph

```
untangle graph ./src --lang ruby --format dot > deps.dot
```

**Outputs:** Raw dependency graph in DOT or JSON format for external tooling (NetworkX, Graphviz, etc).

### `config` — Inspect resolved configuration

```
untangle config show
untangle config explain high_fanout
```

**Outputs:** Resolved configuration with provenance, or an explanation of a specific rule category.

### `service-graph` — Cross-service dependency analysis

```
untangle service-graph . --format json
```

**Outputs:** Cross-service dependency edges derived from GraphQL/OpenAPI usage.

### Global Options

| Flag | Description |
|------|-------------|
| `--lang <LANG>` | `python`, `ruby`, `go`, `rust`. Auto-detect if omitted (inspect file extensions). |
| `--exclude <GLOB>` | Exclude paths (e.g. `--exclude "vendor/**" --exclude "test/**"`) |
| `--include <GLOB>` | Restrict analysis to matching paths |
| `--format <FMT>` | `json` (default), `text`, `dot`, `sarif` |
| `--quiet` | Machine-readable output only, no progress/decoration/timing |

### Runtime Reporting

Timing and throughput are reported as follows:

- **`analyze` (text):** Appends `Completed in ...` to the report.
- **`analyze` (json/sarif):** Includes `elapsed_ms` and `modules_per_second` in output.
- **`analyze` (dot):** Prints a completion line to stderr (unless `--quiet`).
- **`diff` (text/json):** Includes `elapsed_ms` and `modules_per_second` in output; text format ends with `Completed in ...`.
- **`graph` / `service-graph`:** Do not report timing or throughput.

`modules_per_second` is computed as `node_count / (elapsed_ms / 1000)`. For `diff`, it reflects the total modules analyzed across both revisions.

---

## Output Specifications

### `analyze` JSON output

```json
{
  "metadata": {
    "language": "python",
    "granularity": "module",
    "root": "/abs/path/to/src",
    "node_count": 342,
    "edge_count": 1208,
    "edge_density": 0.0103,
    "files_parsed": 358,
    "files_skipped": 2,
    "unresolved_imports": 14,
    "timestamp": "2026-02-14T10:30:00Z",
    "elapsed_ms": 847,
    "modules_per_second": 403.8
  },
  "summary": {
    "mean_fanout": 3.53,
    "p90_fanout": 8,
    "max_fanout": 23,
    "mean_fanin": 3.53,
    "p90_fanin": 9,
    "max_fanin": 41,
    "scc_count": 4,
    "largest_scc_size": 12,
    "total_nodes_in_sccs": 29,
    "max_depth": 7,
    "avg_depth": 4.2,
    "total_complexity": 1557
  },
  "hotspots": [
    {
      "node": "src/core/engine.py",
      "fanout": 23,
      "fanin": 2,
      "entropy": 4.12,
      "scc_id": null,
      "scc_adjusted_entropy": 4.12,
      "fanout_edges": [
        {
          "to": "src/db/connection.py",
          "source_locations": [{ "file": "src/core/engine.py", "line": 3 }]
        },
        {
          "to": "src/utils/logging.py",
          "source_locations": [{ "file": "src/core/engine.py", "line": 7 }]
        }
      ]
    }
  ],
  "sccs": [
    {
      "id": 0,
      "size": 12,
      "members": ["src/models/a.py", "src/models/b.py", "..."],
      "internal_edges": 18
    }
  ]
}
```

### `diff` JSON output

```json
{
  "base_ref": "origin/main",
  "head_ref": "HEAD",
  "verdict": "fail",
  "reasons": ["fanout-increase", "new-scc"],
  "elapsed_ms": 1243,
  "modules_per_second": 387.2,
  "summary_delta": {
    "nodes_added": 3,
    "nodes_removed": 1,
    "edges_added": 12,
    "edges_removed": 4,
    "net_edge_change": 8,
    "scc_count_delta": 1,
    "largest_scc_size_delta": 4,
    "mean_fanout_delta": 0.31,
    "mean_entropy_delta": 0.08,
    "max_depth_delta": 1,
    "total_complexity_delta": 27
  },
  "new_edges": [
    {
      "from": "src/api/handler.py",
      "to": "src/db/models.py",
      "source_locations": [
        { "file": "src/api/handler.py", "line": 47 },
        { "file": "src/api/handler.py", "line": 112 }
      ]
    }
  ],
  "removed_edges": [],
  "fanout_changes": [
    {
      "node": "src/api/handler.py",
      "fanout_before": 5,
      "fanout_after": 9,
      "delta": 4,
      "entropy_before": 2.32,
      "entropy_after": 3.17,
      "new_targets": [
        {
          "to": "src/db/models.py",
          "source_locations": [{ "file": "src/api/handler.py", "line": 47 }]
        },
        {
          "to": "src/db/queries.py",
          "source_locations": [{ "file": "src/api/handler.py", "line": 48 }]
        }
      ]
    }
  ],
  "scc_changes": {
    "new_sccs": [
      { "members": ["src/api/handler.py", "src/db/models.py", "src/api/middleware.py"], "size": 3 }
    ],
    "enlarged_sccs": [],
    "resolved_sccs": []
  }
}
```

### SARIF output

For GitHub Advanced Security / Code Scanning integration, emit results as SARIF where each hotspot or regression is a "result" with location, message, and severity.

---

## CI Integration & Exit Codes

| Code | Meaning |
|------|---------|
| `0` | Analysis complete, no policy violations |
| `1` | One or more `--fail-on` conditions triggered, or an error occurred |

### `--fail-on` conditions

| Condition | Triggers when |
|-----------|--------------|
| `fanout-increase` | Any module's fan-out increased |
| `fanout-threshold` | Any module's fan-out exceeds `--threshold-fanout` |
| `new-scc` | A new non-trivial SCC appeared |
| `scc-growth` | An existing SCC gained members |
| `entropy-increase` | Graph-level mean entropy increased |
| `new-edge` | Any new dependency edge was added (strict mode) |

Multiple conditions can be combined: `--fail-on fanout-increase,new-scc`.

---

## Configuration File (`.untangle.toml`)

```toml
[defaults]
lang = "python"
exclude = ["vendor/**", "**/test/**", "**/*_test.go"]

[thresholds]
max_fanout = 15
max_scc_size = 5

[fail_on]
conditions = ["fanout-increase", "new-scc", "scc-growth"]

[python]
# Treat each directory with __init__.py as a module, or go file-level
granularity = "module"  # "module" | "file"
resolve_relative = true

[ruby]
# Attempt Zeitwerk autoload resolution
zeitwerk = true
load_path = ["lib", "app"]

[go]
# Package-level is the only meaningful granularity
# Exclude stdlib from graph
exclude_stdlib = true
```

---

## Language-Specific Parsing Constraints

### Python

| Concern | Detail |
|---------|--------|
| **Dynamic imports** | `importlib.import_module()`, `__import__()` — cannot be statically resolved. Log a warning, skip the edge. |
| **Conditional imports** | `try/except ImportError` — include both branches conservatively. |
| **Relative imports** | `from . import x` — must resolve against package structure. Requires `__init__.py` detection or src layout heuristics. |
| **Namespace packages** | PEP 420 implicit namespace packages (no `__init__.py`) — must handle. |
| **Star imports** | `from x import *` — creates an edge to `x` but actual symbols are unknown. Flag as a warning. |
| **Third-party vs internal** | Only graph internal project modules. Detect boundary via `pyproject.toml`, `setup.cfg`, or heuristic (files under root). |

### Ruby

| Concern | Detail |
|---------|--------|
| **`require` with string interpolation** | `require "#{path}/foo"` — unresolvable. Warn and skip. |
| **`require_relative`** | Resolves relative to the calling file. Straightforward. |
| **Zeitwerk autoloading** | Rails/Zeitwerk maps `CamelCase` constants to `snake_case` file paths. Optional resolution mode. |
| **Bundler/Gemfile** | Only graph project source, not gems. Detect project boundary via `Gemfile` or specified root. |
| **`autoload`** | `autoload :Foo, "path/to/foo"` — explicit mapping, parseable. |
| **Load path ambiguity** | `require "foo"` resolves against `$LOAD_PATH`. Config must specify project load paths. |

### Go

| Concern | Detail |
|---------|--------|
| **Module boundary** | `go.mod` defines the module path. Only graph packages under this module. |
| **Stdlib exclusion** | Exclude standard library imports by default (configurable). |
| **Vendor directory** | If `vendor/` exists, resolve vendored deps or exclude them. |
| **Build tags** | `//go:build linux` — files may be conditionally compiled. Include all by default, optionally filter by target. |
| **Generated code** | `_generated.go`, `*.pb.go` — optionally exclude. |
| **Internal packages** | Go's `internal/` convention restricts visibility. The graph should still include these edges. |
| **Test files** | `_test.go` files may import additional packages. Exclude by default. |

---

## Failure Modes & Edge Cases

### Parse Failures

| Failure | Handling |
|---------|----------|
| Syntax error in source file | Warn, skip the file, continue. Report count of skipped files. |
| Encoding issues (non-UTF8) | Attempt detection, fall back to skip + warn. |
| Circular symlinks in file tree | Detect via visited-inode tracking, skip + warn. |
| Empty project (no parseable files) | Exit code 1 with clear error message. |

### Git / Diff Failures

| Failure | Handling |
|---------|----------|
| Base ref doesn't exist | Exit code 1, message: "Could not resolve base ref: {ref}" |
| Uncommitted changes in work tree | Warn that head analysis uses working tree state. |
| Binary files / submodules | Skip, they contain no import statements. |
| Merge conflicts present | Exit code 1, cannot parse conflicted files. |

### Graph Anomalies

| Situation | Handling |
|-----------|----------|
| Disconnected graph components | Report component count in metadata. Not an error. |
| Self-loops (module imports itself) | Include in graph, flag as warning. |
| Extremely large graph (>10k nodes) | Performance concern. Ensure O(V+E) algorithms. Stream output. |
| Unresolvable import (target not in project) | Exclude edge. Track as "external dependency" in metadata. |

### CI-Specific Concerns

| Concern | Mitigation |
|---------|-----------|
| Flaky results from non-deterministic file ordering | Sort all collections deterministically. |
| Timeout in large monorepos | `--include` scoping. Lazy parsing (stop after import section). |
| Git checkout overhead for `diff` | Use `git show REF:path` to read file contents without full checkout when possible. |
| Baseline drift (base branch changes between pipeline start and diff) | Lock the base ref SHA at pipeline start, pass SHA not branch name. |

---

## Performance Requirements

| Metric | Target |
|--------|--------|
| Parse throughput | ≥10,000 files/sec for import extraction (Rust parser advantage) |
| Memory | O(V + E) — graph held in memory. ~100 bytes/node, ~32 bytes/edge. 10k-node graph ≈ 1–2 MB. |
| `diff` command wall time | <5s for typical PR (<100 changed files) against a 10k-file repo |
| `analyze` wall time | <10s for 10k-file repo |

---

## Architecture Notes (Implementation Guidance)

```
┌──────────────────────────────────────────────────┐
│                    CLI (clap)                     │
├──────────────────────────────────────────────────┤
│              Command Dispatcher                   │
│   analyze │ diff │ graph │ config │ service-graph │
├──────────┬──────┴──────┬─────────────────────────┤
│  Python  │    Ruby     │    Go    │    Rust        │
│  Parser  │   Parser    │  Parser  │   Parser       │
│          │             │          │                │
│  (tree-  │  (tree-     │  (tree-  │  (tree-        │
│  sitter) │  sitter)    │  sitter) │  sitter)       │
├──────────┴─────────────┴─────────────────────────┤
│          Common Graph IR (petgraph)               │
├──────────────────────────────────────────────────┤
│  Metrics Engine                                   │
│  - fan-out/fan-in                                │
│  - Tarjan SCC                                    │
│  - Shannon entropy                               │
│  - delta computation                             │
├──────────────────────────────────────────────────┤
│  Output Formatters (JSON, Text, DOT, SARIF)      │
└──────────────────────────────────────────────────┘
```

**Key crate choices:**

- `tree-sitter` + language grammars for parsing (fast, incremental, handles broken syntax gracefully)
- `graphql-parser` + `serde_yaml` for service schema parsing (GraphQL/OpenAPI)
- `petgraph` for graph data structure and algorithms (Tarjan's SCC is built-in)
- `clap` for CLI argument parsing
- `serde` / `serde_json` for output serialization
- `git2` (libgit2 bindings) for reading files at specific git refs without checkout
- `globset` for path filtering

**Why tree-sitter over regex or full AST parsing:** Tree-sitter produces a concrete syntax tree even for files with syntax errors (partial parse). This is critical for robustness — a single broken file should never crash the entire analysis. It also gives consistent cross-language API for the import extraction layer.

**Graph IR design for future extensibility:** Each parser frontend produces `Vec<RawImport>` per file, which the graph builder resolves into edges on the common `petgraph::DiGraph<GraphNode, GraphEdge>`. The `GraphNode.kind` discriminator (`Module` | `Function` | `Service` | `Endpoint`) and `GraphEdge.source_locations` fields are present in v1 but only `Module` nodes are emitted in code-level analysis. Adding function-level granularity later requires only changes to the parser frontends (extracting function boundaries and resolving which imports are referenced within each function body), not the metrics engine or output formatters.

---

## Non-Goals (Explicit Exclusions)

- **Runtime dependency analysis** — this is static analysis only. No tracing, no profiling.
- **Third-party dependency graphing** — only project-internal modules. Use `cargo-depgraph`, `pipdeptree`, etc. for external deps.
- **Function/symbol-level granularity (v1)** — deferred to a future version. The IR supports it (see Granularity Strategy) but v1 ships module-level only. Line-number provenance on edges provides sufficient actionability for CI gating without the complexity of intra-file scope analysis.
- **Refactoring suggestions** — the tool reports metrics and regressions. It does not suggest fixes.
- **IDE integration** — CLI + CI first. LSP/IDE plugins are a future concern.
- **Incremental/cached analysis** — each run is a clean analysis. Caching is a future optimisation.

---

## Success Criteria

The tool is successful when a team can add a single CI step:

```yaml
- name: Dependency structure check
  run: |
    untangle diff --base origin/main --head ${{ github.sha }} \
      --fail-on fanout-increase,new-scc \
      --format sarif > untangle.sarif
```

...and get actionable, zero-false-positive feedback on whether a PR made the dependency graph worse, with specific edges and modules called out, in under 10 seconds.
