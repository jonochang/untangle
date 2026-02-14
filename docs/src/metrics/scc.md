# Strongly Connected Components (SCCs)

## Definition

A **strongly connected component** (SCC) is a maximal set of modules where every module can reach every other module through directed dependency edges. In practical terms, an SCC represents a **circular dependency cluster**.

## Detection

Untangle uses **Tarjan's algorithm** to find all SCCs in the dependency graph in linear time. Only **non-trivial** SCCs (size >= 2) are reported, since every single module is trivially strongly connected with itself.

## Metrics

| Metric | Level | Description |
|--------|-------|-------------|
| `scc_count` | Graph | Number of non-trivial SCCs |
| `largest_scc_size` | Graph | Size of the largest SCC |
| `total_nodes_in_sccs` | Graph | Total modules involved in circular dependencies |
| `internal_edges` | Per-SCC | Number of edges within the SCC |

## Why SCCs Matter

Circular dependencies are the hardest form of structural debt to unwind:

- **Build order**: Circular clusters cannot be built independently
- **Change propagation**: Changing any module in the cycle may require changes to all other members
- **Testing**: Modules in a cycle are hard to test in isolation
- **Understanding**: Circular dependencies obscure the logical architecture

## SCC Growth in Diffs

The `diff` command tracks three types of SCC changes:

| Change | Description |
|--------|-------------|
| **New SCC** | A circular dependency that didn't exist in the base |
| **Enlarged SCC** | An existing cluster gained new members |
| **Resolved SCC** | A circular dependency was broken |

SCCs are matched between base and head using Jaccard similarity of their member sets (threshold: 0.5).

## Example

```
A → B → C → A    ← SCC #0 (size=3)
    B → D         ← D is not in the SCC
```

Module D depends on B but B doesn't transitively depend on D, so D is not part of the cycle.
