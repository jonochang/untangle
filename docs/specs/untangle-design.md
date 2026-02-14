# Untangle — Technical Design (Rust)

## Overview

This document describes the implementation plan for `untangle`, a Rust CLI tool that builds module-level dependency graphs from Python, Ruby, and Go source code and computes structural complexity metrics. It is the companion to the requirements specification and covers project structure, crate selection, parsing strategy, data model, algorithms, testing approach, and build/release pipeline.

---

## Project Structure

```
untangle/
├── Cargo.toml
├── Cargo.lock
├── .untangle.toml                    # dogfood config
├── deny.toml                       # cargo-deny config
├── cliff.toml                      # git-cliff changelog config
├── src/
│   ├── main.rs                     # entry point, clap setup
│   ├── cli/
│   │   ├── mod.rs
│   │   ├── analyze.rs              # analyze command handler
│   │   ├── diff.rs                 # diff command handler
│   │   └── graph.rs                # graph export command handler
│   ├── parse/
│   │   ├── mod.rs                  # ParseFrontend trait
│   │   ├── common.rs               # RawImport, SourceLocation types
│   │   ├── python.rs               # Python import extraction
│   │   ├── ruby.rs                 # Ruby require extraction
│   │   ├── go.rs                   # Go import extraction
│   │   └── resolver.rs             # Import path → graph node resolution
│   ├── graph/
│   │   ├── mod.rs                  # DepGraph type (wraps petgraph)
│   │   ├── builder.rs              # RawImport[] → DepGraph construction
│   │   ├── diff.rs                 # Graph delta computation
│   │   └── ir.rs                   # GraphNode, GraphEdge, NodeKind
│   ├── metrics/
│   │   ├── mod.rs
│   │   ├── fanout.rs               # fan-out, fan-in computation
│   │   ├── entropy.rs              # Shannon entropy, SCC-adjusted entropy
│   │   ├── scc.rs                  # Tarjan SCC wrapper, SCC metadata
│   │   └── summary.rs              # Aggregate stats (mean, p90, max)
│   ├── output/
│   │   ├── mod.rs                  # OutputFormat enum, dispatch
│   │   ├── json.rs                 # JSON serialisation
│   │   ├── text.rs                 # Human-readable text
│   │   ├── dot.rs                  # Graphviz DOT format
│   │   └── sarif.rs                # SARIF for GitHub code scanning
│   ├── git.rs                      # git2 integration — read files at refs
│   ├── config.rs                   # .untangle.toml loading (serde + toml)
│   ├── walk.rs                     # File discovery, glob filtering, symlink detection
│   └── errors.rs                   # Error types (thiserror)
├── tests/
│   ├── fixtures/
│   │   ├── python/
│   │   │   ├── simple_project/     # basic imports
│   │   │   ├── relative_imports/   # from . import x
│   │   │   ├── namespace_pkg/      # no __init__.py
│   │   │   ├── dynamic_imports/    # importlib usage
│   │   │   ├── star_imports/       # from x import *
│   │   │   ├── circular/           # A→B→C→A
│   │   │   └── syntax_error/       # broken .py file
│   │   ├── ruby/
│   │   │   ├── require_relative/
│   │   │   ├── zeitwerk_project/
│   │   │   ├── interpolated_require/
│   │   │   └── load_path/
│   │   ├── go/
│   │   │   ├── simple_module/
│   │   │   ├── internal_pkg/
│   │   │   ├── build_tags/
│   │   │   ├── vendor/
│   │   │   └── generated_code/
│   │   └── mixed/                  # multi-language edge case
│   ├── integration/
│   │   ├── analyze_test.rs         # end-to-end analyze command
│   │   ├── diff_test.rs            # end-to-end diff command (uses git fixtures)
│   │   ├── graph_test.rs           # export format validation
│   │   └── ci_exit_codes_test.rs   # verify exit code semantics
│   └── snapshots/                  # insta snapshot files
├── benches/
│   ├── parse_bench.rs              # per-language parse throughput
│   └── graph_bench.rs              # metrics computation on large graphs
└── scripts/
    ├── gen_fixtures.sh             # generate large synthetic projects
    └── gen_go_module.sh            # scaffold go.mod test fixtures
```

---

## Crate Dependencies

### Core

| Crate | Purpose | Notes |
|-------|---------|-------|
| `clap` (derive) | CLI argument parsing | Derive API for subcommands, global options |
| `petgraph` | Graph data structure + algorithms | `DiGraph`, `tarjan_scc()`, topological sort |
| `tree-sitter` | Incremental parser runtime | C library with Rust bindings |
| `tree-sitter-python` | Python grammar | |
| `tree-sitter-ruby` | Ruby grammar | |
| `tree-sitter-go` | Go grammar | |
| `serde` + `serde_json` | Serialisation | Output formats, config loading |
| `toml` | Config parsing | `.untangle.toml` |
| `git2` | libgit2 bindings | Read files at arbitrary refs without checkout |
| `globset` | Glob pattern matching | `--include`, `--exclude` |
| `thiserror` | Error type derivation | |
| `miette` | Diagnostic error reporting | Rich terminal output with source spans |

### Supporting

| Crate | Purpose | Notes |
|-------|---------|-------|
| `rayon` | Parallel file parsing | `par_iter()` over file list for parse step |
| `walkdir` | Recursive directory traversal | Handles symlink cycle detection |
| `ignore` | .gitignore-aware walking | Alternative to walkdir — respects `.gitignore` automatically |
| `tracing` + `tracing-subscriber` | Structured logging | Warn on unresolvable imports, skipped files |
| `indicatif` | Progress bars | Gated behind `--quiet` |

### Dev / Test

| Crate | Purpose | Notes |
|-------|---------|-------|
| `insta` | Snapshot testing | Golden-file tests for JSON/DOT/SARIF output |
| `assert_cmd` | CLI integration testing | Run binary, assert stdout/stderr/exit code |
| `predicates` | Assertion helpers | Combine with `assert_cmd` |
| `tempfile` | Temp directories for test repos | |
| `criterion` | Benchmarking | Parse throughput, metrics on synthetic graphs |
| `proptest` | Property-based testing | Graph invariant verification |
| `pretty_assertions` | Readable test diffs | |

### Lint / Quality

| Tool | Purpose | Notes |
|------|---------|-------|
| `cargo-deny` | Dependency audit | License compliance, advisory DB, duplicate detection |
| `cargo-mutants` | Mutation testing | Verify test suite catches real bugs |
| `cargo-llvm-cov` | Code coverage | LLVM-based source coverage |
| `cargo-nextest` | Test runner | Parallel execution, better output, JUnit XML for CI |

---

## Data Model

### Parser Output (common across languages)

```rust
/// Raw import extracted from a single source file.
/// This is the parser frontend's output contract.
struct RawImport {
    /// The import path as written in source (e.g., "from ..models import User")
    raw_path: String,

    /// The source file containing the import
    source_file: PathBuf,

    /// Line number of the import statement
    line: usize,

    /// Column (optional, for SARIF precision)
    column: Option<usize>,

    /// Classification of the import
    kind: ImportKind,

    /// Parser confidence — did we fully resolve this?
    confidence: ImportConfidence,
}

enum ImportKind {
    /// `import foo` / `require "foo"` / `import "foo"`
    Direct,
    /// `from foo import bar` (Python)
    FromImport { module: String, names: Vec<String> },
    /// `from . import foo` (Python)
    RelativeImport { level: usize, names: Vec<String> },
    /// `require_relative "./foo"` (Ruby)
    RequireRelative,
    /// `autoload :Foo, "path"` (Ruby)
    Autoload { constant: String },
}

enum ImportConfidence {
    /// Fully resolved to a project-internal target
    Resolved,
    /// Likely external (third-party / stdlib)
    External,
    /// Contains dynamic component — unresolvable
    Dynamic,
    /// String interpolation or metaprogramming
    Unresolvable,
}
```

### Graph IR

```rust
use petgraph::graph::DiGraph;

type DepGraph = DiGraph<GraphNode, GraphEdge>;

struct GraphNode {
    id: NodeId,
    kind: NodeKind,
    /// Canonical path relative to project root
    path: PathBuf,
    /// Human-readable name (e.g., "src.api.handler" for Python)
    name: String,
    /// Line span — populated for Function nodes (future)
    span: Option<(usize, usize)>,
}

#[derive(Clone, Copy)]
enum NodeKind {
    Module,   // v1
    Function, // future
}

struct GraphEdge {
    /// All import statements that contributed to this edge
    source_locations: Vec<SourceLocation>,
    /// Number of distinct import statements (edge weight for entropy)
    weight: usize,
}

struct SourceLocation {
    file: PathBuf,
    line: usize,
    column: Option<usize>,
}
```

### Metrics Output

```rust
struct AnalysisResult {
    metadata: Metadata,
    summary: Summary,
    hotspots: Vec<NodeMetrics>,
    sccs: Vec<SccInfo>,
    diagnostics: Vec<Diagnostic>,
    timing: Timing,
}

struct NodeMetrics {
    node: String,
    fanout: usize,
    fanin: usize,
    entropy: f64,
    scc_id: Option<usize>,
    scc_adjusted_entropy: f64,
    fanout_edges: Vec<EdgeDetail>,
}

struct DiffResult {
    base_ref: String,
    head_ref: String,
    verdict: Verdict,
    reasons: Vec<String>,
    summary_delta: SummaryDelta,
    new_edges: Vec<EdgeDetail>,
    removed_edges: Vec<EdgeDetail>,
    fanout_changes: Vec<FanoutChange>,
    scc_changes: SccChanges,
    timing: Timing,
}

struct Timing {
    /// Wall-clock time in milliseconds
    elapsed_ms: u64,
    /// node_count / (elapsed_ms / 1000)
    modules_per_second: f64,
}

enum Verdict {
    Pass,
    Fail,
}
```

---

## Parser Frontend Design

### Trait Interface

```rust
trait ParseFrontend {
    /// Return the tree-sitter Language for this frontend
    fn language(&self) -> tree_sitter::Language;

    /// Extract raw imports from a single file's CST
    fn extract_imports(
        &self,
        tree: &tree_sitter::Tree,
        source: &[u8],
        file_path: &Path,
    ) -> Vec<RawImport>;

    /// Resolve a raw import path to a canonical project-internal node path.
    /// Returns None if the import is external/unresolvable.
    fn resolve(
        &self,
        raw: &RawImport,
        project_root: &Path,
        config: &LanguageConfig,
    ) -> Option<PathBuf>;
}
```

### Tree-sitter Query Strategy

Each language uses an S-expression query compiled once at startup.

**Python:**
```scheme
;; Matches: import foo, import foo.bar
(import_statement
  name: (dotted_name) @import_path)

;; Matches: from foo import bar, from foo.bar import baz
(import_from_statement
  module_name: (dotted_name) @module
  name: (dotted_name) @imported_name)

;; Matches: from . import foo (relative)
(import_from_statement
  module_name: (relative_import) @relative_module
  name: (dotted_name) @imported_name)
```

**Go:**
```scheme
;; Matches: import "path/to/pkg"
(import_spec
  path: (interpreted_string_literal) @import_path)
```

**Ruby:**
```scheme
;; Matches: require "foo", require_relative "foo"
(call
  method: [(identifier) @method]
  arguments: (argument_list
    (string (string_content) @path)))
  (#match? @method "^require(_relative)?$")

;; Matches: autoload :Foo, "path"
(call
  method: (identifier) @method
  arguments: (argument_list
    (simple_symbol) @constant
    (string (string_content) @path))
  (#eq? @method "autoload"))
```

### Resolution Logic

Each resolver takes a `RawImport` and maps it to a canonical `PathBuf` within the project:

**Python resolver:**
1. Split dotted path on `.`
2. Walk from project root (or `src/` layout), checking for `__init__.py` at each level
3. Handle relative imports by counting leading dots and resolving from the importing file's package
4. Check against known project modules — if no match, classify as `External`

**Go resolver:**
1. Read `go.mod` to get module path prefix
2. If import path starts with module path, strip prefix → relative package directory
3. Otherwise, classify as `External` (or `Stdlib` if in Go stdlib list)

**Ruby resolver:**
1. `require_relative`: resolve relative to requiring file's directory
2. `require`: try each configured `load_path` entry, look for `{path}.rb` or `{path}/init.rb`
3. Zeitwerk mode: convert `CamelCase` constant references to `snake_case` paths
4. If no match in project, classify as `External`

---

## Algorithm Details

### Graph Construction Pipeline

```
Files on disk (or at git ref)
    │
    ▼
File walker (walkdir/ignore, glob filtering)
    │
    ▼
Parallel parse (rayon::par_iter)
    │  For each file:
    │  1. tree-sitter parse → CST
    │  2. extract_imports() → Vec<RawImport>
    │  3. resolve() each import → Option<PathBuf>
    │
    ▼
Vec<(PathBuf, Vec<ResolvedImport>)>
    │
    ▼
Graph builder:
    │  - Create/lookup node for each unique module path
    │  - Create/lookup edge for each resolved import
    │  - Accumulate source_locations on edges
    │
    ▼
DepGraph (petgraph::DiGraph)
```

### Diff Algorithm

```
1. Build graph_base from files at base ref (via git2)
2. Build graph_head from files at head ref (working tree or git ref)
3. Compute node sets: added = head_nodes - base_nodes
                      removed = base_nodes - head_nodes
4. Compute edge sets: For shared nodes, diff adjacency lists
                      new_edges = head_edges - base_edges
                      removed_edges = base_edges - head_edges
5. Run SCC on both graphs
6. Diff SCCs:
   - Map SCCs by member overlap (Jaccard similarity > 0.5 = same SCC evolved)
   - new_sccs = head SCCs with no base match
   - resolved_sccs = base SCCs with no head match
   - enlarged_sccs = matched SCCs where |head| > |base|
7. Compute per-node metric deltas for changed nodes
8. Apply --fail-on policy → Verdict
```

### Entropy Computation

```rust
fn shannon_entropy(edge_weights: &[usize]) -> f64 {
    let total: f64 = edge_weights.iter().sum::<usize>() as f64;
    if total == 0.0 {
        return 0.0;
    }
    edge_weights
        .iter()
        .filter(|&&w| w > 0)
        .map(|&w| {
            let p = w as f64 / total;
            -p * p.log2()
        })
        .sum()
}

fn scc_adjusted_entropy(base_entropy: f64, scc_size: usize) -> f64 {
    if scc_size <= 1 {
        base_entropy
    } else {
        base_entropy * (1.0 + (scc_size as f64).ln())
    }
}
```

---

## Git Integration

Use `git2` to read file contents at arbitrary refs without checking out branches:

```rust
fn read_file_at_ref(
    repo: &git2::Repository,
    reference: &str,
    path: &Path,
) -> Result<Vec<u8>> {
    let obj = repo.revparse_single(reference)?;
    let commit = obj.peel_to_commit()?;
    let tree = commit.tree()?;
    let entry = tree.get_path(path)?;
    let blob = entry.to_object(repo)?.peel_to_blob()?;
    Ok(blob.content().to_vec())
}
```

For `diff`, enumerate files at each ref using tree walking:

```rust
fn list_files_at_ref(
    repo: &git2::Repository,
    reference: &str,
    extensions: &[&str],
) -> Result<Vec<PathBuf>> {
    let obj = repo.revparse_single(reference)?;
    let tree = obj.peel_to_commit()?.tree()?;
    let mut files = Vec::new();
    tree.walk(git2::TreeWalkMode::PreOrder, |dir, entry| {
        if let Some(name) = entry.name() {
            let path = PathBuf::from(dir).join(name);
            if extensions.iter().any(|ext| name.ends_with(ext)) {
                files.push(path);
            }
        }
        git2::TreeWalkResult::Ok
    })?;
    Ok(files)
}
```

This avoids filesystem checkout entirely. The parse pipeline accepts `&[u8]` source content, not file paths, so both working-tree and git-ref codepaths converge at the same parser interface.

---

## Testing Strategy

### Unit Tests

**Scope:** Individual functions in isolation. Parser extraction, resolution logic, metric computation, entropy math.

**Framework:** Built-in `#[cfg(test)]` modules.

**Examples:**

```rust
#[cfg(test)]
mod tests {
    #[test]
    fn python_extracts_from_import() {
        let source = b"from foo.bar import baz";
        let imports = PythonFrontend::new().parse_source(source, Path::new("test.py"));
        assert_eq!(imports.len(), 1);
        assert_eq!(imports[0].raw_path, "foo.bar");
    }

    #[test]
    fn entropy_uniform_distribution() {
        // 4 equal weights → log2(4) = 2.0
        let h = shannon_entropy(&[1, 1, 1, 1]);
        assert!((h - 2.0).abs() < 1e-10);
    }

    #[test]
    fn entropy_single_dependency() {
        // All weight on one edge → 0 entropy
        let h = shannon_entropy(&[10]);
        assert!((h - 0.0).abs() < 1e-10);
    }

    #[test]
    fn scc_adjusted_trivial() {
        // SCC of size 1 → no adjustment
        assert_eq!(scc_adjusted_entropy(2.0, 1), 2.0);
    }

    #[test]
    fn scc_adjusted_nontrivial() {
        // SCC of size 5 → 2.0 * (1 + ln(5))
        let result = scc_adjusted_entropy(2.0, 5);
        let expected = 2.0 * (1.0 + 5.0_f64.ln());
        assert!((result - expected).abs() < 1e-10);
    }
}
```

### Snapshot Tests (`insta`)

**Scope:** Full output format verification. Run `analyze` or `graph` on a fixture project, snapshot the JSON/DOT/text/SARIF output.

**Why insta:** Snapshot tests catch unintended output changes without manually maintaining expected strings. `cargo insta review` provides an interactive diff workflow for updating snapshots after intentional changes.

```rust
#[test]
fn analyze_python_simple_project_json() {
    let result = run_analyze("tests/fixtures/python/simple_project", "python", "json");
    insta::assert_json_snapshot!(result);
}

#[test]
fn graph_go_module_dot() {
    let result = run_graph("tests/fixtures/go/simple_module", "go", "dot");
    insta::assert_snapshot!(result);
}

#[test]
fn diff_python_circular_added() {
    let result = run_diff_fixture("python_circular_added");
    insta::assert_json_snapshot!(result);
}
```

Snapshot files live in `tests/snapshots/` and are committed to version control.

### Integration Tests (`assert_cmd`)

**Scope:** Full CLI binary execution. Verify exit codes, stdout format, stderr warnings, flag combinations.

```rust
use assert_cmd::Command;
use predicates::prelude::*;

#[test]
fn analyze_returns_zero_on_clean_project() {
    Command::cargo_bin("untangle")
        .unwrap()
        .args(["analyze", "tests/fixtures/python/simple_project", "--lang", "python"])
        .assert()
        .success()
        .stdout(predicate::str::contains("node_count"));
}

#[test]
fn analyze_returns_two_on_empty_project() {
    Command::cargo_bin("untangle")
        .unwrap()
        .args(["analyze", "tests/fixtures/empty/", "--lang", "python"])
        .assert()
        .code(2)
        .stderr(predicate::str::contains("No parseable files"));
}

#[test]
fn diff_fails_on_fanout_increase() {
    // Fixture is a git repo with two commits
    Command::cargo_bin("untangle")
        .unwrap()
        .current_dir("tests/fixtures/python/fanout_increase_repo")
        .args(["diff", "--base", "HEAD~1", "--head", "HEAD",
               "--lang", "python", "--fail-on", "fanout-increase"])
        .assert()
        .code(1)
        .stdout(predicate::str::contains(r#""verdict": "fail"#));
}

#[test]
fn quiet_flag_suppresses_progress() {
    Command::cargo_bin("untangle")
        .unwrap()
        .args(["analyze", "tests/fixtures/python/simple_project",
               "--lang", "python", "--quiet"])
        .assert()
        .success()
        .stderr(predicate::str::is_empty());
}

#[test]
fn sarif_output_is_valid_json() {
    let output = Command::cargo_bin("untangle")
        .unwrap()
        .args(["analyze", "tests/fixtures/python/simple_project",
               "--lang", "python", "--format", "sarif"])
        .output()
        .unwrap();
    let sarif: serde_json::Value = serde_json::from_slice(&output.stdout).unwrap();
    assert_eq!(sarif["$schema"], "https://raw.githubusercontent.com/oasis-tcs/sarif-spec/main/sarif-2.1/schema/sarif-schema-2.1.0.json");
}
```

### Property-Based Tests (`proptest`)

**Scope:** Graph invariant verification. Generate random graphs, verify metrics are mathematically consistent.

```rust
use proptest::prelude::*;

proptest! {
    #[test]
    fn fanout_equals_out_degree(edges in prop::collection::vec((0usize..50, 0usize..50), 0..200)) {
        let graph = build_graph_from_edges(&edges);
        for node in graph.node_indices() {
            let computed_fanout = compute_fanout(&graph, node);
            let actual_out_degree = graph.edges_directed(node, petgraph::Direction::Outgoing).count();
            prop_assert_eq!(computed_fanout, actual_out_degree);
        }
    }

    #[test]
    fn entropy_is_non_negative(weights in prop::collection::vec(1usize..100, 1..20)) {
        let h = shannon_entropy(&weights);
        prop_assert!(h >= 0.0);
    }

    #[test]
    fn entropy_bounded_by_log_n(weights in prop::collection::vec(1usize..100, 1..20)) {
        let h = shannon_entropy(&weights);
        let max_h = (weights.len() as f64).log2();
        prop_assert!(h <= max_h + 1e-10);
    }

    #[test]
    fn scc_members_have_paths_between_them(
        edges in prop::collection::vec((0usize..30, 0usize..30), 0..100)
    ) {
        let graph = build_graph_from_edges(&edges);
        let sccs = petgraph::algo::tarjan_scc(&graph);
        for scc in &sccs {
            if scc.len() > 1 {
                // Every pair in SCC should be mutually reachable
                for &a in scc {
                    for &b in scc {
                        if a != b {
                            prop_assert!(petgraph::algo::has_path_connecting(&graph, a, b, None));
                        }
                    }
                }
            }
        }
    }

    #[test]
    fn diff_edge_counts_are_consistent(
        base_edges in prop::collection::vec((0usize..20, 0usize..20), 0..50),
        head_edges in prop::collection::vec((0usize..20, 0usize..20), 0..50),
    ) {
        let base = build_graph_from_edges(&base_edges);
        let head = build_graph_from_edges(&head_edges);
        let delta = compute_diff(&base, &head);

        // net change = added - removed
        prop_assert_eq!(
            delta.edges_added as isize - delta.edges_removed as isize,
            delta.net_edge_change
        );
    }
}
```

### Benchmarks (`criterion`)

**Scope:** Performance regression detection. Track parse throughput and metrics computation against known-size inputs.

```rust
use criterion::{criterion_group, criterion_main, Criterion, BenchmarkId};

fn bench_python_parse(c: &mut Criterion) {
    let files = collect_files("tests/fixtures/python/simple_project", "py");
    c.bench_function("python_parse_simple", |b| {
        b.iter(|| {
            let frontend = PythonFrontend::new();
            for (path, source) in &files {
                frontend.extract_imports_from_source(source, path);
            }
        })
    });
}

fn bench_scc_on_large_graph(c: &mut Criterion) {
    let mut group = c.benchmark_group("scc_scaling");
    for size in [100, 1000, 5000, 10000] {
        let graph = generate_synthetic_graph(size, size * 3);
        group.bench_with_input(
            BenchmarkId::new("tarjan", size),
            &graph,
            |b, g| b.iter(|| petgraph::algo::tarjan_scc(g)),
        );
    }
    group.finish();
}

fn bench_entropy_computation(c: &mut Criterion) {
    let graph = generate_synthetic_graph(5000, 15000);
    c.bench_function("entropy_all_nodes", |b| {
        b.iter(|| compute_all_entropy(&graph))
    });
}

criterion_group!(benches, bench_python_parse, bench_scc_on_large_graph, bench_entropy_computation);
criterion_main!(benches);
```

### Mutation Testing (`cargo-mutants`)

**Scope:** Test quality verification. Introduce code mutations (e.g., change `>` to `>=`, swap `+` and `-`) and verify the test suite catches them.

```bash
# Run mutation testing on the metrics module
cargo mutants -- -p untangle --lib -- metrics

# Run on the full project (slow — CI nightly only)
cargo mutants
```

**Why this matters:** Mutation testing catches the case where your tests pass but don't actually verify the right thing. For mathematical code (entropy, SCC adjustment), this is especially valuable — an off-by-one in the entropy formula might not show up in simple unit tests but will be caught by mutants.

**Target:** ≥80% mutation kill rate on `metrics/` and `graph/` modules.

### Coverage (`cargo-llvm-cov`)

```bash
# Generate coverage report
cargo llvm-cov --html --open

# Enforce minimum coverage in CI
cargo llvm-cov --fail-under-lines 80
```

### Test Runner (`cargo-nextest`)

```bash
# Run all tests with parallel execution and structured output
cargo nextest run

# Generate JUnit XML for CI
cargo nextest run --profile ci
```

`.config/nextest.toml`:
```toml
[profile.ci]
fail-fast = false
status-level = "fail"

[profile.ci.junit]
path = "target/nextest/ci/results.xml"
```

---

## Test Fixture Strategy

### Static Fixtures (checked into repo)

Small, focused projects in `tests/fixtures/` that exercise specific parser edge cases. Each fixture is the minimum set of files needed to trigger a behaviour.

**Naming convention:** `tests/fixtures/{language}/{scenario}/`

**Each fixture includes a `README.md`** explaining what it tests and the expected graph edges.

### Git Repo Fixtures (for `diff` tests)

The `diff` command needs git history. These fixtures are generated by a setup script and checked in as bare repos:

```bash
#!/bin/bash
# scripts/gen_diff_fixture.sh
# Creates a bare git repo with two commits for diff testing.

dir="tests/fixtures/python/fanout_increase_repo"
mkdir -p "$dir" && cd "$dir"
git init
# commit 1: base state
mkdir -p src/api src/db
echo 'import src.db.connection' > src/api/handler.py
echo '' > src/db/connection.py
git add . && git commit -m "base"
# commit 2: add imports (fan-out increase)
echo -e 'import src.db.connection\nimport src.db.models\nimport src.db.queries' > src/api/handler.py
echo '' > src/db/models.py
echo '' > src/db/queries.py
git add . && git commit -m "add deps"
```

### Synthetic Graph Generation (for benchmarks)

```rust
fn generate_synthetic_graph(nodes: usize, edges: usize) -> DepGraph {
    let mut rng = StdRng::seed_from_u64(42); // deterministic
    let mut graph = DiGraph::new();
    let node_indices: Vec<_> = (0..nodes)
        .map(|i| graph.add_node(GraphNode {
            id: NodeId(i),
            kind: NodeKind::Module,
            path: PathBuf::from(format!("mod_{i}.py")),
            name: format!("mod_{i}"),
            span: None,
        }))
        .collect();
    for _ in 0..edges {
        let from = node_indices[rng.gen_range(0..nodes)];
        let to = node_indices[rng.gen_range(0..nodes)];
        if from != to {
            graph.add_edge(from, to, GraphEdge {
                source_locations: vec![],
                weight: 1,
            });
        }
    }
    graph
}
```

---

## CI Pipeline

```yaml
name: CI
on: [push, pull_request]

jobs:
  check:
    runs-on: ubuntu-latest
    steps:
      - uses: actions/checkout@v4
      - uses: dtolnay/rust-toolchain@stable
        with:
          components: clippy, rustfmt

      - name: Format check
        run: cargo fmt --check

      - name: Clippy
        run: cargo clippy --all-targets -- -D warnings

      - name: Dependency audit
        run: cargo deny check

      - name: Tests
        run: cargo nextest run

      - name: Coverage
        run: |
          cargo llvm-cov --fail-under-lines 80 --codecov --output-path codecov.json
          # Upload to codecov/coveralls

      - name: Benchmarks (regression check)
        run: |
          cargo bench -- --save-baseline pr
          # Compare against main baseline if available

  mutation:
    runs-on: ubuntu-latest
    if: github.event_name == 'push' && github.ref == 'refs/heads/main'
    steps:
      - uses: actions/checkout@v4
      - uses: dtolnay/rust-toolchain@stable
      - name: Mutation testing
        run: cargo mutants -p untangle -- metrics graph

  dogfood:
    runs-on: ubuntu-latest
    needs: check
    steps:
      - uses: actions/checkout@v4
        with:
          fetch-depth: 0 # need full history for diff

      - name: Build
        run: cargo build --release

      - name: Self-analyze
        run: ./target/release/untangle analyze src/ --lang rust --format json
        # Future: once Rust support is added, dogfood on own codebase
```

---

## Build & Release

### Cross-compilation targets

| Target | OS | Notes |
|--------|----|-------|
| `x86_64-unknown-linux-gnu` | Linux | Primary CI/server target |
| `x86_64-unknown-linux-musl` | Linux (static) | For Docker/Alpine |
| `x86_64-apple-darwin` | macOS Intel | |
| `aarch64-apple-darwin` | macOS ARM | |
| `x86_64-pc-windows-msvc` | Windows | |

Use `cross` for cross-compilation or `cargo-zigbuild` for musl targets.

### Release pipeline

```bash
# Tag-triggered release via cargo-dist or release-plz
# Produces:
# - GitHub Release with pre-built binaries
# - Homebrew formula (via tap)
# - Cargo publish (crates.io)
```

### Binary size concerns

Tree-sitter grammars are compiled into the binary. Each grammar adds ~200KB–1MB. Three grammars ≈ 1–3MB overhead. Total binary target: <15MB stripped.

```toml
# Cargo.toml profile
[profile.release]
opt-level = "z"     # size optimisation
lto = true          # link-time optimisation
strip = true        # strip debug symbols
codegen-units = 1   # single codegen unit for better optimisation
```

---

## Implementation Order

### Phase 1: Foundation (week 1)

1. Scaffold project with `clap` CLI skeleton (all three subcommands, no implementation)
2. Implement `GraphNode`, `GraphEdge`, `DepGraph` IR with petgraph
3. Implement Go parser frontend (cleanest import model, fastest to validate)
4. Implement `analyze` command with JSON output for Go
5. Write snapshot tests for Go analysis output

### Phase 2: Metrics & Multi-language (week 2)

6. Implement fan-out, fan-in, entropy, SCC computation
7. Implement Python parser frontend (relative imports, `__init__.py` handling)
8. Implement Ruby parser frontend (`require`, `require_relative`, basic Zeitwerk)
9. Add text and DOT output formatters
10. Property-based tests for metric invariants

### Phase 3: Diff & CI (week 3)

11. Integrate `git2` for reading files at refs
12. Implement `diff` command and graph delta algorithm
13. Implement `--fail-on` policy engine and exit codes
14. Add SARIF output formatter
15. Integration tests for diff with git repo fixtures

### Phase 4: Polish (week 4)

16. Config file loading (`.untangle.toml`)
17. Auto language detection
18. `--include` / `--exclude` glob support
19. Progress indicators (`indicatif`)
20. `cargo-mutants` pass, coverage enforcement
21. Benchmarks, performance tuning
22. Cross-platform release builds

---

## Open Questions — Resolution Plan

Five design questions must be resolved before or during Phase 1. Each question below includes the decision method, who/what resolves it, the timeline, and the recommended default.

---

### Q1: Edge weight — import count or binary?

**Question:** When module A has `from B import x, y, z`, is the edge weight 3 (one per imported name) or 1 (binary: edge exists or it doesn't)?

**Why it matters:** This directly changes entropy values. Import-count weighting means a module that imports 10 names from one dependency has a different entropy profile than one that imports 1 name from each of 10 dependencies. Binary weighting treats both the same.

**Resolution method:** Build both and measure on a real codebase.

**Plan:**

1. **Phase 1, day 2:** Implement the `GraphEdge` with a `weight: usize` field and `source_locations: Vec<SourceLocation>`. The weight is always populated regardless of mode.
2. **Phase 1, day 4:** After the Go parser works end-to-end, run `analyze` on two real Go codebases (one small utility, one medium service) with both weighting modes. Compare:
   - Do the entropy rankings change? (i.e., do different modules become the "worst" hotspot?)
   - Does the import-count mode flag facade modules (`__init__.py`, Go packages that re-export) as artificially high-entropy?
   - Is the delta in `diff` mode more or less stable between commits?
3. **Phase 2, day 1:** Make a decision based on the comparison. Implement as a config flag if the answer is "it depends on the codebase."

**Recommended default:** Binary (0/1). Reasoning: fan-out measures "how many things does this module know about," not "how many names does it import." A module with `from db import (User, Session, Transaction, Query)` depends on `db` — the fact that it imports 4 names is a separate concern (interface surface area) that fan-out isn't designed to capture. Binary weighting produces more stable diffs and is easier to reason about.

**Escape hatch:** Keep the `source_locations` vector on every edge regardless. Users who want import-count weighting can derive it from `source_locations.len()` in post-processing, and we can promote it to a first-class mode later if there's demand.

**Decision gate:** End of Phase 1. Blocking for Phase 2 entropy implementation.

---

### Q2: Python re-exports via `__init__.py`

**Question:** When `pkg/__init__.py` contains `from .submodule import Foo`, and consumer code writes `from pkg import Foo`, should the edge go to `pkg/__init__.py` or to `pkg/submodule.py`?

**Why it matters:** Many Python packages use `__init__.py` as a public API facade. Routing edges to `__init__.py` is technically correct (that's what the import statement says) but makes `__init__.py` a fan-in monster that obscures the real dependency structure. Routing to the submodule is more useful but requires parsing `__init__.py` to resolve re-exports — which starts to look like symbol-level resolution (explicitly out of scope for v1).

**Resolution method:** Design spike with two test fixtures.

**Plan:**

1. **Phase 2, day 1 (Python parser):** Create two test fixtures:
   - `tests/fixtures/python/facade_init/` — a package where `__init__.py` re-exports from submodules
   - `tests/fixtures/python/flat_init/` — a package where `__init__.py` defines its own code
2. **Phase 2, day 1:** Implement the naive approach first: edge always goes to the module that the import statement names (i.e., `__init__.py` for `from pkg import Foo`).
3. **Phase 2, day 2:** Run analysis on both fixtures. Check:
   - Does `__init__.py` dominate the fan-in rankings in the facade case?
   - Does the `diff` output produce false positives when someone adds a re-export to `__init__.py`?
4. **Phase 2, day 2:** If the naive approach is noisy, implement a shallow re-export resolver:
   - Parse `__init__.py` for `from .X import Y` patterns
   - Build a re-export map: `{(pkg, Y) → pkg.X}`
   - When resolving `from pkg import Y`, check the re-export map first
   - This is *not* full symbol resolution — it's a single-level lookup in `__init__.py` only

**Recommended default:** Start naive (edge to `__init__.py`), add shallow re-export resolution only if the noise is unacceptable. The shallow resolver is ~50 lines of code and handles the 90% case (simple `from .X import Y` in `__init__.py`).

**What we explicitly won't do:** Chase re-exports through multiple levels, resolve `__all__`, or handle dynamic re-exports. That's symbol resolution territory.

**Decision gate:** Phase 2, day 2. Non-blocking for Phase 1.

---

### Q3: `--fail-on fanout-increase` scope — all nodes or changed files only?

**Question:** If a PR refactors module A (reducing its fan-out) but as a side effect module B (untouched in the PR) now has higher fan-out because a dependency was split, should `--fail-on fanout-increase` fire?

**Why it matters:** This is the single biggest determinant of false-positive rate in CI. All-nodes catches indirect structural effects but will flag PRs that are net-positive refactors. Changed-files-only is less noisy but can miss structural regressions.

**Resolution method:** Implement both modes, test on real PR histories, pick the right default.

**Plan:**

1. **Phase 3, day 1 (diff command):** Implement the diff engine with three reporting scopes:
   - `all` — report fan-out changes for every node in the graph
   - `changed` — report only for nodes whose source files were modified in the PR
   - `affected` — report for modified files AND their direct dependents (one hop)
2. **Phase 3, day 2:** Create three test fixtures simulating common PR patterns:
   - **Pure addition:** New module added, imports existing modules. Only the new file has fan-out changes.
   - **Refactor split:** Large module split into two. Fan-out shifts to consumers that now import from two modules instead of one.
   - **Dependency chain extension:** A→B becomes A→B→C. A is untouched but its transitive dependency depth increased.
3. **Phase 3, day 3:** Wire up `--fail-on-scope all|changed|affected` flag. Default TBD based on fixture testing.

**Recommended default:** `affected` (changed files + one hop). Reasoning:

- `all` is too noisy. A routine refactor that splits a module will flag every consumer, even though the overall structure improved. This produces "cry wolf" CI failures that teams learn to ignore.
- `changed` is too narrow. If I add a circular dependency between two existing modules by editing only one of them, the other module's SCC membership changed but `changed` wouldn't flag it.
- `affected` is the pragmatic middle. It catches direct structural regressions without flagging the entire graph. One hop covers the "I added an import to module A that creates a cycle involving module B" case.

**Escape hatch:** All three modes are always computed internally. The `--fail-on-scope` flag just controls which scope triggers a non-zero exit code. The JSON output always includes all changes so users can build their own policies.

**Decision gate:** Phase 3, day 3. Blocking for CI integration.

---

### Q4: Ruby Zeitwerk resolution strategy

**Question:** For Rails apps using Zeitwerk autoloading, should we resolve constant references (`UserController`) to file paths (`app/controllers/user_controller.rb`) based on directory structure conventions alone, or should we parse Rails config files (`config/application.rb`, `config/initializers/`) to discover custom autoload paths and inflection rules?

**Why it matters:** Zeitwerk's default behaviour is purely conventional (CamelCase → snake_case, namespace → directory). But Rails apps can configure custom inflections (`HTMLParser` → `html_parser.rb` instead of `h_t_m_l_parser.rb`) and add non-standard autoload paths. Ignoring config means some edges won't resolve; parsing config means going down a Rails-specific rabbit hole.

**Resolution method:** Scope the problem by surveying real Rails apps, then implement the minimum viable resolver.

**Plan:**

1. **Phase 2, day 3 (Ruby parser):** Implement Zeitwerk resolution using only default conventions:
   - `CamelCase` → `snake_case` via standard Ruby inflection rules
   - Look up in configured `load_path` entries (from `.untangle.toml`)
   - No config file parsing
2. **Phase 2, day 3:** Create test fixtures:
   - `tests/fixtures/ruby/zeitwerk_project/` — standard Rails-like structure
   - `tests/fixtures/ruby/zeitwerk_custom_inflection/` — app with `HTMLParser` style constants
3. **Phase 2, day 4:** Test against the fixtures. Measure unresolved import rate.
4. **If unresolved rate >15%:** Add a `.untangle.toml` config section for manual inflection overrides:

   ```toml
   [ruby.zeitwerk]
   inflections = { "HTML" = "html", "API" = "api", "OAuth" = "oauth" }
   extra_autoload_paths = ["app/services", "app/lib"]
   ```

   This lets users declare their custom inflections without us parsing Ruby config files.

**Recommended default:** Convention-only resolution + manual config overrides. No Rails config file parsing.

**Why not parse Rails config:** The config files are Ruby code. Parsing them correctly means evaluating Ruby (or at minimum, pattern-matching on `ActiveSupport::Inflector` calls). This is fragile, version-dependent, and a maintenance burden disproportionate to the value. The manual override in `.untangle.toml` takes 30 seconds for a user to write and handles every case.

**What we explicitly won't do:**
- Parse `config/application.rb` or `config/initializers/inflections.rb`
- Support Zeitwerk's `collapse` or `ignore` directives (these can be approximated with `--exclude` globs)
- Resolve `autoload :Foo, "custom_path"` where the path is computed dynamically

**Decision gate:** Phase 2, day 4. Non-blocking for Go and Python parsers.

---

### Q5: Go test file handling

**Question:** Should `_test.go` files be included in the dependency graph, excluded entirely, or included as a separate layer?

**Why it matters:** Go test files often import test utilities, assertion libraries, and mock packages that create high fan-out noise. Including them inflates fan-out metrics and can trigger false `--fail-on fanout-increase` failures for routine test additions. But excluding them entirely means you can't detect circular dependencies between test code and production code (which are real problems in Go).

**Resolution method:** Implement exclude-by-default with a flag, validate on real Go projects.

**Plan:**

1. **Phase 1, day 3 (Go parser):** Implement test file detection:
   - Files matching `*_test.go` are tagged as `FileKind::Test`
   - The file walker respects a `--include-tests` flag (default: off)
   - When tests are excluded, they don't contribute nodes or edges to the graph
2. **Phase 1, day 4:** Create test fixtures:
   - `tests/fixtures/go/simple_module/` — includes both production and test files
   - Run `analyze` with and without `--include-tests`
   - Snapshot both outputs
3. **Phase 3, day 2:** When diff is implemented, verify that adding a test file doesn't trigger `--fail-on fanout-increase` when tests are excluded.
4. **Deferred (future version):** Consider a dual-layer graph mode where test nodes and production nodes coexist but are visually/metrically separated. This would allow detecting "test imports production in a cycle" without polluting production metrics.

**Recommended default:** Exclude `_test.go` by default. `--include-tests` to opt in.

**Reasoning:**
- The primary use case is CI gating on production dependency structure. Test dependencies are a secondary concern.
- Go test files can import the package they're testing via `package foo_test` (external test package) or `package foo` (internal test). The internal variant creates edges that are technically circular (production package ↔ test file in same package) but are a Go convention, not a structural problem.
- Excluding by default means zero noise on day one. Teams that want test dependency analysis can opt in.

**Dual-layer graph (future):** The graph IR already supports `NodeKind`. Adding a `Test` variant is trivial. The harder question is how to report metrics — separate summaries for production and test subgraphs, or a combined view with test nodes visually distinct? This is a UX question better answered after the tool has real users.

**Decision gate:** Phase 1, day 3. Implemented as part of Go parser, resolved before Phase 2.

---

### Resolution Timeline Summary

| Question | Resolves in | Blocks | Method |
|----------|------------|--------|--------|
| Q1: Edge weight | Phase 1, end | Phase 2 entropy | Build both, measure on real code |
| Q2: `__init__.py` re-exports | Phase 2, day 2 | Nothing (start naive) | Spike with fixtures, add shallow resolver if noisy |
| Q3: `--fail-on` scope | Phase 3, day 3 | CI integration | Implement three modes, test on PR patterns |
| Q4: Zeitwerk resolution | Phase 2, day 4 | Nothing (convention-first) | Convention-only + config overrides |
| Q5: Go test files | Phase 1, day 3 | Nothing (exclude by default) | Flag + fixtures, validate on real Go projects |

**Critical path:** Only Q1 is on the critical path (blocks entropy implementation in Phase 2). All others have safe defaults that can be shipped and refined later.

**Decision log:** Each resolution should be recorded as a brief ADR (Architecture Decision Record) in `docs/decisions/` with the question, alternatives considered, decision, and rationale. This prevents re-litigating the same questions when new contributors join.
