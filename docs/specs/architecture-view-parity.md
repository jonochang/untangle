# Architecture View Parity

This note records the comparison between `arch-view`'s Clojure implementation and `untangle`'s Rust `architecture` command.

## Matched

- Hierarchy projection is based on namespace or path segments rather than raw files.
- Multiple module edges between the same projected components are aggregated into a single architecture edge with a count.
- Cycle handling uses SCC detection first, then removes a feedback-edge set before assigning layers.
- Layer assignment is computed on the acyclic remainder of the projected graph.
- Feedback edges are preserved in output so cyclic relationships remain visible after ranking.

## Adapted For `untangle`

- `untangle` derives hierarchy from logical module names first, then falls back to paths across Python, Ruby, Go, and Rust instead of relying on Clojure namespaces alone.
- Boilerplate source roots such as `src`, `lib`, `app`, and `pkg` are stripped during projection to approximate `arch-view`'s dropped top-level namespace segment.
- Output is non-interactive JSON or DOT rather than a browser UI.
- Aggregated edges carry `source_location_count` from the underlying `untangle` graph so the projection still surfaces provenance density.

## Deferred

- Interactive drill-down and source browsing from `arch-view` are not ported.
- Abstract-vs-concrete node classification is not represented yet.
- Mixed leaf handling from the UI model is not exposed as a separate public concept in v1.
