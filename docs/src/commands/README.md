# Commands

Untangle provides four subcommands:

| Command | Purpose |
|---------|---------|
| [`analyze`](./analyze.md) | Analyze a source directory and report metrics |
| [`diff`](./diff.md) | Compare dependency graphs between git revisions |
| [`graph`](./graph.md) | Export the raw dependency graph (DOT or JSON) |
| [`config`](./config.md) | Inspect resolved configuration and provenance |

All commands respect the [configuration system](../configuration/README.md), which merges defaults, config files, environment variables, and CLI flags.
