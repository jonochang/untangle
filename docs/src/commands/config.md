# config

Inspect the resolved configuration and understand where each setting comes from.

## Usage

```bash
untangle config <SUBCOMMAND>
```

## Subcommands

### show

Display the fully resolved configuration with provenance information.

```bash
untangle config show [--path <DIR>]
```

This shows every configuration value and its source (built-in default, user config, project config, environment variable, or CLI flag).

### explain

Explain where a specific rule's thresholds come from.

```bash
untangle config explain <CATEGORY> [--path <DIR>]
```

Where `CATEGORY` is one of:
- `high_fanout`
- `god_module`
- `circular_dependency`
- `deep_chain`
- `high_entropy`

## Options

| Flag | Type | Description |
|------|------|-------------|
| `--path` | directory | Working directory (defaults to current directory). Used to locate the nearest `.untangle.toml`. |

## Examples

### Show all resolved configuration

```bash
untangle config show
```

### Show configuration for a specific project

```bash
untangle config show --path /path/to/project
```

### Explain fan-out rule provenance

```bash
untangle config explain high_fanout
```

This will show the current values and which configuration layer set each one (e.g., "min_fanout = 10, set by project config at /path/to/.untangle.toml").
