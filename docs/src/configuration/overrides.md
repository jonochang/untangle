# Per-Path Overrides

Overrides let you apply different rules to different parts of your codebase using glob patterns.

## Syntax

```toml
[overrides."<glob-pattern>"]
enabled = true|false
rules.<rule>.<field> = <value>
```

## How It Works

1. For each module, untangle checks overrides in order
2. **First matching glob wins** — subsequent patterns are not checked
3. If `enabled = false`, the module is excluded from insights entirely
4. If `rules` are specified, they **replace** the base rules (unspecified fields revert to built-in defaults, not the project config values)

## Examples

### Disable analysis for vendor code

```toml
[overrides."**/vendor/**"]
enabled = false
```

### Relax thresholds for legacy code

```toml
[overrides."src/legacy/**"]
rules.high_fanout.min_fanout = 40
rules.high_fanout.relative_to_p90 = false
```

### Strict rules for core modules

```toml
[overrides."src/core/**"]
rules.high_fanout.min_fanout = 3
rules.god_module.min_fanout = 2
rules.god_module.min_fanin = 2
```

## Override Semantics

When an override matches:

- **`enabled = false`**: The module is skipped for all insight rules. It still appears in the graph and metrics, but no insights are generated.
- **`rules` block**: The entire rule set is replaced. Only explicitly set fields are non-default. For example:

```toml
[overrides."src/legacy/**"]
rules.high_fanout.min_fanout = 40
```

This sets `high_fanout.min_fanout = 40` but also resets `high_fanout.relative_to_p90` to its built-in default (`true`), not the project config value. Other rules (god_module, circular_dependency, etc.) also revert to defaults within the override.

## Ordering

Place more specific patterns before more general ones:

```toml
# This won't work as expected — src/** matches first
[overrides."src/**"]
enabled = false

[overrides."src/legacy/**"]
enabled = true  # Never reached!
```

Instead:

```toml
# Put the more specific pattern first
[overrides."src/legacy/**"]
rules.high_fanout.min_fanout = 40

[overrides."src/**"]
enabled = false
```

## Glob Syntax

Overrides use [globset](https://docs.rs/globset/) patterns:

| Pattern | Matches |
|---------|---------|
| `*.py` | Python files in any directory |
| `src/**` | Everything under `src/` |
| `**/vendor/**` | Any `vendor` directory at any depth |
| `src/legacy/*.rs` | Rust files directly in `src/legacy/` |
