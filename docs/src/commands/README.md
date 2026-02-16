# Commands

Untangle provides five subcommands:

| Command | Purpose |
|---------|---------|
| [`analyze`](./analyze.md) | Analyze a source directory and report metrics |
| [`diff`](./diff.md) | Compare dependency graphs between git revisions |
| [`graph`](./graph.md) | Export the raw dependency graph (DOT or JSON) |
| [`config`](./config.md) | Inspect resolved configuration and provenance |
| [`service-graph`](./service-graph.md) | Analyze cross-service dependencies |

All commands read the [configuration system](../configuration/README.md). `service-graph` specifically uses the `[services]` section and does not apply include/exclude/ignore patterns.
