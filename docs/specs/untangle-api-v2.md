# Untangle API Design v2

This proposal is based on the issues captured in `FINDINGS.md` and the current CLI implementation in `src/cli/*`.

The main change is to make `analyze` the single structural-analysis surface, move projections under it, and make format support explicit per command so invalid combinations fail at argument parsing time instead of later at runtime.

## Goals

- Organize the CLI around user intent instead of internal projections.
- Remove globally advertised formats that are not actually supported everywhere.
- Standardize path and targeting behavior across commands.
- Keep machine-readable output stable and explicit.
- Preserve `service-graph` as a separate domain-specific command.

## Non-goals

- Changing the underlying graph or metric algorithms.
- Merging cross-service analysis into module-level analysis.
- Adding function-level structural graphs in this iteration.

## Design Principles

1. One structural entry point.
   `analyze` is the structural command. Raw graph export and architecture projection are views of the same pipeline, not separate top-level products.

2. Parse-time validation.
   Each command or view owns its own format enum. Unsupported values should be rejected by clap before execution starts.

3. Consistent targeting.
   Analysis commands accept `[path]` as an optional positional argument and default to `.`.

4. Shared arguments are shared types.
   Targeting and common runtime flags should be defined once and flattened into the clap structs that need them.

5. JSON is a public API.
   JSON output should advertise a stable `kind` and `schema_version` so downstream automation can safely branch on output type.

## Proposed CLI

### Top-level commands

```bash
untangle analyze [path] [OPTIONS]
untangle analyze graph [path] [OPTIONS]
untangle analyze architecture [path] [OPTIONS]
untangle diff [path] --base <REF> --head <REF> [OPTIONS]
untangle quality functions [path] [OPTIONS]
untangle quality project [path] [OPTIONS]
untangle service-graph [path] [OPTIONS]
untangle config show [path]
untangle config explain <CATEGORY> [path]
```

### Why nested `analyze` views instead of separate top-level commands

`graph` and `architecture` still exist conceptually, but as nested views under `analyze` rather than sibling commands. This keeps one structural-analysis surface while still allowing each view to define its own flags and formats cleanly.

This is slightly more explicit than `--view graph` or `--view architecture`, but it is easier to implement cleanly with clap because:

- `analyze` can remain the default report view
- `analyze graph` can expose only `dot|json`
- `analyze architecture` can expose only `dot|json`
- view-specific flags like `--level` stay local to the architecture projection

## Command Details

### `untangle analyze`

Default structural report for the current tree.

```bash
untangle analyze [path] \
  [--lang <LANG>] \
  [--format text|json|sarif] \
  [--top <N>] \
  [--threshold-fanout <N>] \
  [--threshold-scc <N>] \
  [--insights auto|on|off] \
  [--include-tests] \
  [--include <GLOB>...] \
  [--exclude <GLOB>...] \
  [--quiet]
```

Notes:

- `path` defaults to `.`
- `--insights` replaces the asymmetric `--no-insights` flag
- `dot` is not accepted here because raw graph export is a different view

Examples:

```bash
untangle analyze
untangle analyze . --lang python --format text
untangle analyze . --format sarif --threshold-fanout 15
```

### `untangle analyze graph`

Raw dependency graph export.

```bash
untangle analyze graph [path] \
  [--lang <LANG>] \
  [--format dot|json] \
  [--include-tests] \
  [--include <GLOB>...] \
  [--exclude <GLOB>...] \
  [--quiet]
```

Examples:

```bash
untangle analyze graph . --format dot | dot -Tsvg -o deps.svg
untangle analyze graph . --format json > graph.json
```

### `untangle analyze architecture`

Projected architecture view over the same dependency graph.

```bash
untangle analyze architecture [path] \
  [--lang <LANG>] \
  [--format dot|json] \
  [--level <N>] \
  [--include-tests] \
  [--include <GLOB>...] \
  [--exclude <GLOB>...] \
  [--quiet]
```

Examples:

```bash
untangle analyze architecture . --level 2 --format dot
untangle analyze architecture . --format json
```

### `untangle diff`

Compare structural state between two refs.

```bash
untangle diff [path] \
  --base <REF> \
  --head <REF> \
  [--lang <LANG>] \
  [--format text|json] \
  [--fail-on <POLICY,POLICY,...>] \
  [--include-tests] \
  [--include <GLOB>...] \
  [--exclude <GLOB>...] \
  [--quiet]
```

Notes:

- `sarif` should not be exposed until it is genuinely implemented for diff
- `dot` is not accepted because diff is a report, not a graph export

### `untangle quality functions`

Function-level quality metrics.

```bash
untangle quality functions [path] \
  --coverage <FILE> \
  [--lang <LANG>] \
  [--format text|json] \
  [--metric crap] \
  [--top <N>] \
  [--min-cc <N>] \
  [--min-score <N>] \
  [--include-tests] \
  [--include <GLOB>...] \
  [--exclude <GLOB>...] \
  [--quiet]
```

Notes:

- `functions` is the subject users care about
- `--metric crap` remains for forward compatibility, but only valid function metrics belong here
- `--coverage` is required until a no-coverage function metric exists

### `untangle quality project`

Project-level quality summary that combines structural and function metrics.

```bash
untangle quality project [path] \
  --coverage <FILE> \
  [--lang <LANG>] \
  [--format text|json] \
  [--top <N>] \
  [--min-cc <N>] \
  [--min-score <N>] \
  [--include-tests] \
  [--include <GLOB>...] \
  [--exclude <GLOB>...] \
  [--quiet]
```

This replaces the current `quality --metric overall` mode with a clearer user-facing noun.

### `untangle service-graph`

Keep this separate.

```bash
untangle service-graph [path] [--format text|json|dot]
```

Notes:

- `path` defaults to `.`
- this command continues to rely on `[services]` config and service metadata
- no attempt is made to fold it under `analyze`

### `untangle config`

```bash
untangle config show [path]
untangle config explain <CATEGORY> [path]
```

This removes the `--path` inconsistency and matches the rest of the CLI.

## Shared Argument Model

The CLI should be composed from shared argument structs instead of repeating flags across commands.

```rust
#[derive(Debug, Clone, Args)]
pub struct TargetArgs {
    pub path: Option<PathBuf>,
    #[arg(long)]
    pub lang: Option<Language>,
    #[arg(long)]
    pub include_tests: bool,
    #[arg(long)]
    pub include: Vec<String>,
    #[arg(long)]
    pub exclude: Vec<String>,
}

#[derive(Debug, Clone, Args)]
pub struct RuntimeArgs {
    #[arg(long)]
    pub quiet: bool,
}
```

Then each command adds only its own truly local options.

## Format Model

Replace the global `OutputFormat` enum with per-command enums:

```rust
pub enum AnalyzeReportFormat {
    Text,
    Json,
    Sarif,
}

pub enum GraphFormat {
    Dot,
    Json,
}

pub enum ArchitectureFormat {
    Dot,
    Json,
}

pub enum DiffFormat {
    Text,
    Json,
}

pub enum QualityFormat {
    Text,
    Json,
}

pub enum ServiceGraphFormat {
    Text,
    Json,
    Dot,
}
```

This makes the help output truthful and removes late runtime format errors.

## Config Model

The current global `defaults.format` should be replaced with command-specific defaults.

### Proposed config shape

```toml
[defaults]
lang = "python"
quiet = false
include_tests = false

[targeting]
include = ["src/**"]
exclude = ["vendor/**"]

[analyze.report]
format = "text"
top = 20
insights = "auto"
threshold_fanout = 15
threshold_scc = 4

[analyze.graph]
format = "dot"

[analyze.architecture]
format = "dot"
level = 2

[diff]
format = "json"
fail_on = ["fanout-increase", "new-scc"]

[quality.functions]
format = "text"
metric = "crap"
top = 20
min_cc = 2
min_score = 0

[quality.project]
format = "text"
top = 20
min_cc = 2
min_score = 0

[service_graph]
format = "json"
```

### Migration rules

- `defaults.format` becomes deprecated legacy fallback
- existing `graph` defaults move to `[analyze.graph]`
- existing `architecture` defaults move to `[analyze.architecture]`
- existing `quality` defaults split across `[quality.functions]` and `[quality.project]`
- existing `[services]` declarations stay unchanged

## JSON Contract

All JSON outputs should advertise a stable output kind and schema version.

### Common JSON header

```json
{
  "kind": "analyze.report",
  "schema_version": 2,
  "metadata": {}
}
```

Proposed `kind` values:

- `analyze.report`
- `analyze.graph`
- `analyze.architecture`
- `diff.report`
- `quality.functions`
- `quality.project`
- `service_graph`

Rules:

- `kind` is mandatory for every JSON output
- `schema_version` is mandatory for every JSON output
- text and dot remain human-facing formats and are not versioned the same way
- output-specific payload fields continue to live beside `metadata`

This keeps machine consumers from inferring output shape from the command line that produced it.

## Internal Rust API

If the CLI is refactored toward a reusable library surface, the public request and response types should follow the same command model.

```rust
pub struct TargetSpec {
    pub root: PathBuf,
    pub lang: Option<Language>,
    pub include_tests: bool,
    pub include: Vec<String>,
    pub exclude: Vec<String>,
}

pub struct Snapshot {
    pub graph: DepGraph,
    pub metadata: SnapshotMetadata,
}

pub fn snapshot(target: &TargetSpec) -> Result<Snapshot>;
pub fn analyze_report(snapshot: &Snapshot, opts: AnalyzeReportOptions) -> AnalysisReport;
pub fn export_graph(snapshot: &Snapshot) -> GraphExport;
pub fn project_architecture(snapshot: &Snapshot, opts: ArchitectureOptions) -> ArchitectureExport;
pub fn diff(request: DiffRequest) -> Result<DiffReport>;
pub fn quality_functions(request: QualityFunctionsRequest) -> Result<QualityFunctionsReport>;
pub fn quality_project(request: QualityProjectRequest) -> Result<QualityProjectReport>;
pub fn service_graph(request: ServiceGraphRequest) -> Result<ServiceGraphReport>;
```

The important implementation property is that structural graph construction happens once and different views are derived from the same snapshot instead of each command rebuilding its own ad hoc pipeline.

## Compatibility and Rollout

### Phase 1

- add the new `analyze graph` and `analyze architecture` forms
- add `quality functions` and `quality project`
- support `[path]` on `config show` and `config explain`
- keep existing `graph`, `architecture`, and `quality --metric overall` as deprecated aliases

### Phase 2

- remove deprecated commands from help output
- emit deprecation warnings for legacy config keys like `defaults.format`

### Phase 3

- remove legacy aliases in the next breaking release

## Command Mapping

| Current | Proposed |
|--------|----------|
| `untangle analyze <PATH>` | `untangle analyze [path]` |
| `untangle graph <PATH>` | `untangle analyze graph [path]` |
| `untangle architecture <PATH>` | `untangle analyze architecture [path]` |
| `untangle diff [PATH] --base ... --head ...` | `untangle diff [path] --base ... --head ...` |
| `untangle quality <PATH> --metric crap` | `untangle quality functions [path] --metric crap` |
| `untangle quality <PATH> --metric overall` | `untangle quality project [path]` |
| `untangle config show --path <DIR>` | `untangle config show [path]` |
| `untangle config explain <CATEGORY> --path <DIR>` | `untangle config explain <CATEGORY> [path]` |

## Recommendation

Adopt the command model above as the v2 public API.

The highest-value implementation order is:

1. move `graph` and `architecture` under `analyze`
2. replace global format handling with per-command enums
3. standardize `[path]` defaults across commands
4. split `quality` into `functions` and `project`
5. add JSON `kind` and `schema_version`
