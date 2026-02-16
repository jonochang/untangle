# Architecture

This page provides an overview of the internal module structure for contributors.

## High-Level Architecture

```
CLI (clap)
  └─ Commands: analyze, diff, graph, config, service-graph
       └─ Config resolution (5-layer merge)
       └─ File discovery (walkdir + globset)
       └─ Parsing (tree-sitter, per-language frontends)
       └─ Graph building (petgraph)
       └─ Metrics computation
       └─ Insight generation
       └─ Output formatting (JSON, text, DOT, SARIF)
```

## Module Map

### `src/cli/`

Command implementations. Each subcommand lives in its own file:

| File | Description |
|------|-------------|
| `analyze.rs` | `AnalyzeArgs`, file discovery, parallel parsing, graph building, output |
| `diff.rs` | `DiffArgs`, `FailCondition`, git ref graph building, diff computation, policy evaluation |
| `graph.rs` | `GraphArgs`, DOT/JSON graph export |
| `config.rs` | `ConfigArgs`, `ConfigAction::Show/Explain` |
| `service_graph.rs` | `ServiceGraphArgs`, cross-service dependency detection |

### `src/config/`

Layered configuration system:

| File | Description |
|------|-------------|
| `mod.rs` | `ResolvedConfig`, `ResolvedRules`, all rule structs with defaults |
| `schema.rs` | `FileConfig` — TOML-deserializable with all-Option fields |
| `resolve.rs` | `CliOverrides`, `resolve_config()` — 5-layer merge logic |
| `provenance.rs` | `Source` enum, `ProvenanceMap` — tracks where each value came from |
| `overrides.rs` | `apply_overrides()` — per-path glob rule overrides |
| `ignore.rs` | `.untangleignore` loading and pattern parsing |
| `show.rs` | `render_show()`, `render_explain()` — config display |

### `src/parse/`

Language frontends using tree-sitter:

| File | Description |
|------|-------------|
| `common.rs` | `RawImport`, `SourceLocation`, `ImportConfidence`, `ParseFrontend` trait |
| `python.rs` | `PythonFrontend` — `import`/`from` extraction |
| `ruby.rs` | `RubyFrontend` — `require`/`require_relative` extraction |
| `go.rs` | `GoFrontend` — `import` declarations, `go.mod` module path |
| `rust.rs` | `RustFrontend` — `use` statements, scoped imports, `Cargo.toml` |
| `graphql.rs` | GraphQL schema parsing |
| `graphql_client.rs` | GraphQL client usage detection |
| `openapi.rs` | OpenAPI spec parsing |
| `rest_client.rs` | REST client usage detection |

Each frontend implements the `ParseFrontend` trait:
- `extract_imports()` — parse source bytes into `Vec<RawImport>`
- `resolve()` — resolve a `RawImport` to a project-internal module path

### `src/graph/`

Dependency graph data structures:

| File | Description |
|------|-------------|
| `ir.rs` | `DepGraph` type alias, `GraphNode`, `GraphEdge`, `NodeKind` |
| `builder.rs` | `GraphBuilder`, `ResolvedImport` — accumulates edges, deduplicates |
| `diff.rs` | `DiffResult`, `Verdict`, `SummaryDelta`, change types |

### `src/metrics/`

Graph metrics computation:

| File | Description |
|------|-------------|
| `fanout.rs` | `fan_out()`, `fan_in()` — per-node degree |
| `entropy.rs` | `shannon_entropy()`, `scc_adjusted_entropy()` |
| `scc.rs` | `find_non_trivial_sccs()`, `node_scc_map()`, `SccInfo` |
| `depth.rs` | `max_depth()`, `avg_depth()` on condensation DAG |
| `summary.rs` | `Summary` — aggregate statistics (mean, p90, max) |

### `src/output/`

Output formatters:

| File | Description |
|------|-------------|
| `json.rs` | `AnalyzeOutput`, `Metadata`, `Hotspot`, `write_analyze_json()`, `write_diff_json()` |
| `text.rs` | `write_analyze_text()` — human-readable report |
| `dot.rs` | `write_dot()` — Graphviz DOT format |
| `sarif.rs` | `write_sarif()` — SARIF 2.1.0 output |

### Other Modules

| File | Description |
|------|-------------|
| `src/insights.rs` | `Insight`, `InsightCategory`, `InsightSeverity`, `generate_insights()`, `generate_insights_with_config()` |
| `src/walk.rs` | `Language`, `discover_files()`, `detect_language()` |
| `src/git.rs` | `open_repo()`, `list_files_at_ref()`, `read_file_at_ref()` |
| `src/errors.rs` | `UntangleError`, `Result` type alias |

## Key Design Decisions

### Tree-sitter for parsing

Tree-sitter provides error-tolerant parsing, meaning untangle can handle files with syntax errors (partial parse). It also runs at native speed with zero GC pauses.

**Caveat:** `Parser` is not `Send`, so parallel parsing creates a fresh parser per Rayon thread.

### petgraph for the dependency graph

petgraph provides efficient graph data structures with Tarjan's SCC algorithm. The graph uses `DiGraph<GraphNode, GraphEdge>` where edges carry source locations and weights.

### Layered configuration

The 5-layer config system (defaults < user < project < env < CLI) gives maximum flexibility while maintaining a clear, debuggable precedence order. The `ProvenanceMap` makes it easy to understand where any value came from.

### Condensation DAG for depth

Depth is computed on the condensation (SCCs collapsed to single nodes) to ensure it's well-defined for cyclic graphs.

## Testing Strategy

- **Unit tests**: Per-module, co-located with source
- **Integration tests**: `tests/` directory, using `assert_cmd` for CLI testing
- **Property-based tests**: Using `proptest` for graph metric properties
- **Snapshot tests**: Using `insta` for output format stability
- **Benchmarks**: Using `criterion` for parse and graph performance
