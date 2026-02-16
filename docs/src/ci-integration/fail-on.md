# Fail-on Conditions

The `--fail-on` flag specifies which structural regressions should cause `untangle diff` to exit with code `1`.

## Available Conditions

| Condition | Triggers When |
|-----------|---------------|
| `fanout-increase` | Any module's fan-out increased between base and head |
| `fanout-threshold=N` | Any module's fan-out exceeds N in the head |
| `new-scc` | A new circular dependency cluster appeared |
| `scc-growth` | An existing circular cluster gained members |
| `entropy-increase` | Graph-level mean entropy increased |
| `new-edge` | Any new dependency edge was added (strict mode) |

## Usage

### CLI

```bash
# Single condition
untangle diff --base origin/main --head HEAD --fail-on new-scc

# Multiple conditions (comma-separated)
untangle diff --base origin/main --head HEAD \
  --fail-on fanout-increase,new-scc,scc-growth

# Threshold condition
untangle diff --base origin/main --head HEAD \
  --fail-on fanout-threshold=15
```

### Config File

```toml
[fail_on]
conditions = ["fanout-increase", "new-scc", "scc-growth"]
```

### Environment Variable

```bash
UNTANGLE_FAIL_ON="fanout-increase,new-scc" untangle diff --base origin/main --head HEAD
```

## Condition Details

### `fanout-increase`

Triggers if **any** module's fan-out is higher in head than in base. This is the most common CI gate — it prevents gradual coupling increase.

### `fanout-threshold=N`

Triggers if **any** module's fan-out exceeds N in the head revision. Unlike `fanout-increase`, this checks absolute values rather than deltas. Useful for enforcing a hard ceiling.

### `new-scc`

Triggers if there are SCCs in the head that don't match any SCC in the base (using Jaccard similarity > 0.5 for matching). This catches new circular dependencies being introduced.

### `scc-growth`

Triggers if an existing SCC (matched by Jaccard similarity) has grown in size. A cycle that started as 3 modules and grew to 5 triggers this condition.

### `entropy-increase`

Triggers if the graph-level mean entropy increased. This is a broad indicator of growing coupling.

### `new-edge`

Triggers if **any** new dependency edge was added. This is the strictest mode — useful for locked-down modules where no new dependencies should be introduced.

## Recommended Combinations

| Use Case | Conditions |
|----------|-----------|
| Standard CI gate | `fanout-increase,new-scc` |
| Strict CI gate | `fanout-increase,new-scc,scc-growth,entropy-increase` |
| Lock mode | `new-edge` |
| Absolute limits | `fanout-threshold=20,new-scc` |
