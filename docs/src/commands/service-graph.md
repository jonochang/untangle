# service-graph

Analyze cross-service dependencies using the `[services]` section in `.untangle.toml`.

The command scans each service's source code for GraphQL and REST client usage, then matches those usages to known GraphQL schemas and OpenAPI specs.

## Usage

```bash
untangle service-graph <PATH> [--format json|text|dot]
```

## Arguments

| Argument | Description |
|----------|-------------|
| `PATH` | Path to the project root (required) |

## Options

| Flag | Type | Description |
|------|------|-------------|
| `--format` | `json\|text\|dot` | Output format. Default: `json`. |

## Configuration

`service-graph` requires a `[services]` section in `.untangle.toml`:

```toml
[services."web"]
root = "services/web"
lang = "python"
graphql_schemas = ["services/web/schema.graphql"]
openapi_specs = ["services/web/openapi.yaml"]
base_urls = ["https://web.internal"]

[services."billing"]
root = "services/billing"
lang = "go"
openapi_specs = ["services/billing/openapi.yaml"]
base_urls = ["https://billing.internal"]
```

Notes:
- Paths are resolved relative to the project root.
- `lang` is optional. If omitted, Untangle auto-detects languages for that service root.
- `service-graph` currently scans all files under each service root and does not apply `.untangleignore` or include/exclude globs.

## Output

The output contains:

- `services`: the configured services with file counts
- `cross_service_edges`: edges with `kind` (`graphql_query` or `rest_call`), optional `operation`, and `source_locations`

See [JSON Format](../output-formats/json.md) for the schema.

## Examples

### Text output

```bash
untangle service-graph . --format text
```

### DOT graph for visualization

```bash
untangle service-graph . --format dot | dot -Tsvg -o service-graph.svg
```
