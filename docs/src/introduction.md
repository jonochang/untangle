# Introduction

Untangle is a fast, multi-language dependency graph analyzer that catches structural regressions in CI.

It builds **module-level dependency graphs** from your source code, computes structural complexity metrics, and diffs them between git revisions. Add it to your CI pipeline to fail PRs that introduce circular dependencies or increase coupling.

## Why Untangle?

As codebases grow, dependency structure degrades silently. New imports get added, modules accumulate responsibilities, and circular dependencies form. By the time you notice, untangling the mess is expensive.

Untangle makes structural complexity **visible and measurable**, letting you:

- **Gate PRs** on structural health (no new circular dependencies, no fan-out explosions)
- **Track trends** in coupling, depth, and entropy over time
- **Identify hotspots** — modules that are central hubs or tightly coupled clusters
- **Export graphs** for visualization and architecture reviews

## Supported Languages

| Language | Granularity | What It Parses |
|----------|-------------|----------------|
| Python   | Module (file-level) | `import` / `from ... import` |
| Ruby     | File-level | `require` / `require_relative` |
| Go       | Package-level | `import` declarations, `go.mod` |
| Rust     | Module-level | `use` statements, `Cargo.toml` |

All languages are parsed using [tree-sitter](https://tree-sitter.github.io/tree-sitter/), which means untangle handles files with syntax errors gracefully (partial parse) and runs at thousands of files per second.

## How It Works

1. **Discover** source files matching the target language
2. **Parse** each file with tree-sitter to extract import statements
3. **Resolve** imports to project-internal modules (filtering out external/stdlib)
4. **Build** a directed dependency graph using [petgraph](https://docs.rs/petgraph/)
5. **Compute** metrics: fan-out, fan-in, entropy, SCCs, depth
6. **Report** results in JSON, text, DOT, or SARIF format

For `diff` mode, untangle reads files at arbitrary git refs via [libgit2](https://libgit2.org/) without checking out branches, keeping the operation fast and non-destructive.
