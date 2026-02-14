# Entropy

## Shannon Entropy

Untangle computes Shannon entropy on the outgoing edge weight distribution of each module:

```
H = -Σ pᵢ log₂(pᵢ)
```

where `pᵢ = wᵢ / Σw` is the proportion of total outgoing weight on edge `i`.

### What It Measures

Entropy distinguishes between modules that depend on many things equally vs. modules that depend on many things but concentrate on one. For example:

| Scenario | Fan-out | Entropy |
|----------|---------|---------|
| 10 dependencies, all used equally | 10 | 3.32 (log₂(10)) |
| 10 dependencies, 90% of weight on one | 10 | 0.47 |

Both have the same fan-out, but very different structural risk profiles. The first is genuinely spread across 10 concerns; the second is really about one dependency with 9 minor ones.

### Properties

- Entropy is `0` when a module has 0 or 1 outgoing edges
- Entropy reaches `log₂(n)` when all `n` outgoing edges have equal weight
- Higher entropy means more evenly distributed dependencies

## SCC-Adjusted Entropy

For modules inside a non-trivial [strongly connected component](./scc.md) (circular dependency cluster), entropy is amplified:

```
H_adj = H × (1 + ln(|SCC|))
```

where `|SCC|` is the number of modules in the SCC.

### Rationale

Dependencies within a circular cluster are more entangled than simple directed dependencies. The amplification factor reflects that:

- An SCC of size 2: multiplier = `1 + ln(2) ≈ 1.69`
- An SCC of size 5: multiplier = `1 + ln(5) ≈ 2.61`
- An SCC of size 10: multiplier = `1 + ln(10) ≈ 3.30`

This means a module inside a 10-member circular cluster has its entropy effectively tripled, reflecting the compounded complexity of circular dependencies.
