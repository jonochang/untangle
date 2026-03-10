# config

Inspect the resolved configuration and understand where each setting comes from.

## Usage

```bash
untangle config show [PATH]
untangle config explain <CATEGORY> [PATH]
```

## Subcommands

### show

Display the fully resolved configuration with provenance information.

```bash
untangle config show [PATH]
```

### explain

Explain where a specific rule's thresholds come from.

```bash
untangle config explain <CATEGORY> [PATH]
```

Where `CATEGORY` is one of:

- `high_fanout`
- `god_module`
- `circular_dependency`
- `deep_chain`
- `high_entropy`

## Examples

```bash
untangle config show
untangle config show /path/to/project
untangle config explain high_fanout
untangle config explain high_fanout /path/to/project
```
