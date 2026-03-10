# DOT (Graphviz) Format

DOT is available only on the graph-style commands:

- `untangle analyze graph`
- `untangle analyze architecture`
- `untangle service-graph`

## Usage

```bash
untangle analyze graph ./src --lang go --format dot | dot -Tsvg -o deps.svg
untangle analyze architecture ./src --lang python --format dot | dot -Tsvg -o architecture.svg
untangle service-graph . --format dot | dot -Tsvg -o service-graph.svg
```

## Notes

- `analyze report`, `diff`, and `quality` do not support DOT.
- Raw graph DOT uses a left-to-right layout (`rankdir=LR`).
- Architecture DOT uses a layered top-to-bottom layout (`rankdir=TB`).
