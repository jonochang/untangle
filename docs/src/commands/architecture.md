# architecture

Project the raw dependency graph into a layered architecture view.

This command reuses `untangle`'s normal parsing and resolution pipeline, then groups modules into higher-level components based on their path hierarchy. The projection and layer assignment are modeled on the original Clojure `arch-view` implementation.

## Usage

```bash
untangle architecture <PATH> [OPTIONS]
```

## Options

| Flag | Description |
|------|-------------|
| `--lang <LANG>` | Analyze a single language (`python`, `ruby`, `go`, `rust`) |
| `--format <FMT>` | Output format: `json` or `dot` |
| `--level <N>` | Project to hierarchy depth `N` (default: `1`) |
| `--include-tests` | Include test files |
| `--include <GLOB>` | Include matching files |
| `--exclude <GLOB>` | Exclude matching files |
| `--quiet` | Suppress progress output |

## Output

### JSON

JSON output includes:

- `level`: the selected projection depth
- `metadata`: root path plus source node and edge counts
- `nodes`: projected architecture components with `id`, `label`, `layer`, and `module_count`
- `edges`: aggregated component edges with `count`, `source_location_count`, and `feedback`
- `feedback_edges`: the projected edges removed to rank cyclic graphs
- `layers`: nodes grouped by layer index

### DOT

DOT output renders a top-to-bottom architecture graph:

- `rankdir=TB` for layered output
- one node per projected component
- edge labels for aggregated counts when useful
- feedback edges highlighted in `firebrick` with dashed styling

## Examples

### Top-level architecture

```bash
untangle architecture ./src --lang python --format json
```

### Drill into the second hierarchy level

```bash
untangle architecture ./src --lang ruby --level 2 --format json
```

### Render with Graphviz

```bash
untangle architecture ./src --lang go --format dot | dot -Tsvg -o architecture.svg
```

## Notes

- The projection strips common source-container roots such as `src`, `lib`, `app`, and `pkg` so the output emphasizes architectural components instead of filesystem boilerplate.
- `architecture` only supports `json` and `dot` in this version.
