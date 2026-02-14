# Configuration

Untangle uses a layered configuration system that merges settings from multiple sources with clear precedence rules.

## Configuration Sources (lowest to highest priority)

1. **Built-in defaults** — sensible defaults for all settings
2. **User config** — `~/.config/untangle/config.toml` (personal preferences)
3. **Project config** — `.untangle.toml` in or above the working directory
4. **Environment variables** — `UNTANGLE_*` variables
5. **CLI flags** — command-line arguments

Higher-priority sources override lower-priority ones. See [Resolution Order](./resolution.md) for details.

## Quick Start

Create a `.untangle.toml` in your project root:

```toml
[defaults]
lang = "python"
format = "text"

[targeting]
exclude = ["vendor/**", "**/test/**"]

[rules.high_fanout]
min_fanout = 10

[fail_on]
conditions = ["fanout-increase", "new-scc"]
```

## Sections

- [Config File Reference](./config-file.md) — complete `.untangle.toml` schema
- [Resolution Order](./resolution.md) — how layers are merged
- [Per-Path Overrides](./overrides.md) — different rules for different paths
- [.untangleignore](./untangleignore.md) — gitignore-style file exclusion
- [Environment Variables](./environment-variables.md) — all `UNTANGLE_*` vars

## Inspecting Configuration

Use the `config` command to see the resolved configuration:

```bash
# Show all resolved values with provenance
untangle config show

# Explain where a rule's thresholds come from
untangle config explain high_fanout
```
