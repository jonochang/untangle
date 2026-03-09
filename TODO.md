# Add Layered Architecture View To `untangle`

## Summary
Add a new `untangle architecture` subcommand that produces a non-interactive, layered architecture view for module-level dependency graphs. Reuse `untangle`'s existing parsing, resolution, and graph-building pipeline, then add a new projection layer inspired by `arch-view` that:
- groups modules by hierarchical path segments
- collapses same-level dependencies into aggregated architecture edges
- breaks cycles for ranking while still marking feedback edges
- emits architecture-specific JSON and DOT outputs

The work includes an explicit review/comparison step against `arch-view`'s Clojure implementation before finalizing the Rust behavior, so parity gaps are documented and intentional rather than accidental.

## Key Changes
### Review and parity check
- Start with a focused review of `arch-view`'s relevant Clojure modules against the new Rust implementation:
  - hierarchy projection behavior
  - cycle / feedback-edge selection
  - layer assignment rules
  - edge aggregation semantics
- Produce a short parity checklist in the implementation notes or PR description covering:
  - behaviors matched exactly
  - behaviors adapted for `untangle`
  - behaviors intentionally deferred
- Re-run that comparison after implementation and before merge to verify the Rust output still matches the intended `arch-view` ideas on representative sample graphs.

### CLI and outputs
- Add `architecture` to `src/cli/mod.rs` with args parallel to `graph`:
  - positional `path`
  - optional `--lang`
  - optional `--format json|dot`
  - `--include-tests`, `--include`, `--exclude`, `--quiet`
  - optional `--level <n>` to choose hierarchy depth
    Default: `1`
- Add docs for the new command and update README command lists/examples.
- DOT output should be architecture-oriented:
  - vertical layering (`rankdir=TB`)
  - one node per projected architecture component
  - edges between components
  - visual distinction for feedback-cycle edges
- JSON output should expose a stable architecture projection shape, separate from raw graph JSON.

### Internal architecture projection
- Add a new projection module under `src/graph/` or a new `src/architecture/` area for:
  - hierarchical segmentation of module names/paths into architecture components
  - aggregation from raw module graph to projected nodes and edges
  - cycle detection / feedback-edge selection for layout
  - layer assignment on the acyclic remainder
- Base grouping on `GraphNode.name` segments, using language-appropriate separators already present in names.
  - Python/Ruby: `.` segments
  - Go/Rust: path-like segments normalized into a common segment list
- At projection level `n`, each module maps to the first `n` meaningful hierarchy segments beneath the repo-relative root.
- Aggregate multiple module edges between the same projected nodes into one architecture edge with:
  - edge count
  - feedback-edge flag
  - optional source-location count in JSON
- Keep projection logic independent from CLI so it is unit-testable and reusable later.

### Layout and data types
- Introduce explicit architecture output types, for example:
  - `ArchitectureNode`
  - `ArchitectureEdge`
  - `ArchitectureLayout`
  - `ArchitectureOutput`
- `ArchitectureOutput` should include:
  - selected `level`
  - nodes
  - edges
  - feedback edges
  - layers / `node -> layer`
  - minimal metadata: root path, language(s), source node/edge counts
- Use a deterministic layer algorithm modeled on `arch-view`:
  - normalize edges to in-graph nodes only
  - find SCCs
  - choose feedback edges to remove for ranking
  - compute layers from the remaining DAG
- Do not port `arch-view`'s GUI, source viewer, or recursive drill-down state.
- Do not change existing `graph`, `analyze`, or `service-graph` output schemas in this pass.

## Test Plan
- Add unit tests for projection:
  - modules grouped into expected architecture nodes at level 1 and level 2
  - aggregated edge counts are correct
  - self-edges created by grouping are dropped
  - path/module normalization is deterministic across supported languages
- Add unit tests for layout:
  - acyclic projected graph gets expected layers
  - cyclic projected graph marks feedback edges and still yields layers
  - results are stable for multiple SCC shapes
- Add CLI/integration tests:
  - `untangle architecture <fixture> --format json` returns expected node/edge/layer shape
  - `--level 2` changes grouping as expected
  - DOT output contains top-to-bottom graph settings and projected labels
  - command respects include/exclude/test-file options consistently with `graph`
- Add at least one parity-style test or fixture comparison derived from `arch-view` examples so the Rust projection can be checked against the original Clojure behavior.

## Public Interfaces
- New CLI command: `untangle architecture`
- New output contract:
  - JSON schema distinct from raw `graph --format json`
  - DOT semantics distinct from raw dependency DOT
- No changes to existing command names or existing output schemas.

## Assumptions
- V1 is module-level only; service-level architecture remains future work.
- V1 is static only; no GUI, browser app, or embedded viewer.
- Grouping is derived from existing module naming/path conventions, not from new config files or manual component rules.
- Default view is the top-level architecture (`--level 1`), with deeper levels available via CLI rather than interactive drill-down.
- The review/comparison step is a required acceptance step before the feature is considered complete.
