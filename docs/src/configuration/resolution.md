# Resolution Order

Untangle resolves configuration by applying five layers in order. Each layer can override values from previous layers.

## Precedence (lowest to highest)

```
1. Built-in defaults
2. User config        (~/.config/untangle/config.toml)
3. Project config     (.untangle.toml, walking up from working dir)
4. Environment vars   (UNTANGLE_*)
5. CLI flags          (--format, --lang, etc.)
```

### 1. Built-in Defaults

Every setting has a sensible default. Key defaults:

| Setting | Default |
|---------|---------|
| `format` | `json` |
| `quiet` | `false` |
| `include_tests` | `false` |
| `no_insights` | `false` |
| `go.exclude_stdlib` | `true` |
| `python.resolve_relative` | `true` |
| `ruby.zeitwerk` | `false` |
| `ruby.load_path` | `["lib", "app"]` |
| `rules.high_fanout.min_fanout` | `5` |
| `rules.god_module.min_fanout` | `3` |
| `rules.circular_dependency.warning_min_size` | `4` |
| `rules.deep_chain.absolute_depth` | `8` |
| `rules.high_entropy.min_entropy` | `2.5` |

### 2. User Config

Personal preferences stored at `~/.config/untangle/config.toml`. Useful for setting your preferred output format across all projects.

```toml
[defaults]
format = "text"
```

### 3. Project Config

Project-specific settings in `.untangle.toml`. The file is found by walking up from the working directory to the filesystem root.

```toml
[defaults]
lang = "python"

[targeting]
exclude = ["vendor/**"]

[rules.high_fanout]
min_fanout = 10
```

### 4. Environment Variables

Override any setting via `UNTANGLE_*` environment variables. See [Environment Variables](./environment-variables.md).

```bash
UNTANGLE_FORMAT=text untangle analyze ./src
```

### 5. CLI Flags

Command-line flags have the highest priority and always win.

```bash
untangle analyze ./src --format text --lang python
```

## How Merging Works

- **Scalar values** (strings, booleans, numbers): higher layer replaces lower layer entirely.
- **List values** (include, exclude, fail_on, load_path): higher layer replaces the entire list (not appended).
- **Rule objects**: individual fields within a rule are merged (e.g., setting `min_fanout` in project config doesn't reset `relative_to_p90` from defaults).
- **Overrides**: accumulated from all layers.

## Inspecting Resolution

Use `untangle config show` to see the final resolved value and source of every setting:

```bash
untangle config show
```

Use `untangle config explain <rule>` to trace a specific rule's values through the layers:

```bash
untangle config explain high_fanout
```
