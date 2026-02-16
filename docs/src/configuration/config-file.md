# Config File Reference

The configuration file is a TOML file named `.untangle.toml`, placed in your project root (or any ancestor directory).

User-level configuration lives at `~/.config/untangle/config.toml`.

## Complete Schema

```toml
# ============================================================
# [defaults] — General settings
# ============================================================
[defaults]
lang = "python"           # Language: python, ruby, go, rust
format = "json"           # Output format: json, text, dot, sarif
quiet = false             # Suppress progress output
top = 20                  # Number of top hotspots to report
include_tests = false     # Include test files
no_insights = false       # Suppress insights

# ============================================================
# [targeting] — File inclusion/exclusion
# ============================================================
[targeting]
include = ["src/**"]                           # Only analyze matching paths
exclude = ["vendor/**", "**/test/**"]          # Skip matching paths

# ============================================================
# [rules] — Insight rule configuration
# ============================================================
[rules.high_fanout]
enabled = true            # Enable high fan-out detection
min_fanout = 5            # Minimum fan-out to trigger (default: 5)
relative_to_p90 = true    # Also require fan-out > p90 (default: true)
warning_multiplier = 2    # Fan-out >= N*p90 upgrades to warning (default: 2)

[rules.god_module]
enabled = true            # Enable god module detection
min_fanout = 3            # Minimum fan-out threshold (default: 3)
min_fanin = 3             # Minimum fan-in threshold (default: 3)
relative_to_p90 = true    # Also require > p90 for both (default: true)

[rules.circular_dependency]
enabled = true            # Enable circular dependency detection
warning_min_size = 4      # SCC size >= N upgrades to warning (default: 4)

[rules.deep_chain]
enabled = true            # Enable deep chain detection
absolute_depth = 8        # Always trigger at this depth (default: 8)
relative_multiplier = 2.0 # Trigger if depth > N * avg_depth (default: 2.0)
relative_min_depth = 5    # Minimum depth for relative trigger (default: 5)

[rules.high_entropy]
enabled = true            # Enable high entropy detection
min_entropy = 2.5         # Minimum Shannon entropy (default: 2.5)
min_fanout = 5            # Minimum fan-out to consider (default: 5)

# ============================================================
# [fail_on] — CI failure conditions
# ============================================================
[fail_on]
conditions = ["fanout-increase", "new-scc", "scc-growth"]

# ============================================================
# Language-specific settings
# ============================================================
[go]
exclude_stdlib = true     # Exclude Go standard library imports (default: true)

[python]
resolve_relative = true   # Resolve relative imports (default: true)

[ruby]
zeitwerk = false          # Use Zeitwerk autoload conventions (default: false)
load_path = ["lib", "app"] # Ruby load paths (default: ["lib", "app"])

# ============================================================
# [overrides] — Per-path rule overrides
# ============================================================
[overrides."**/vendor/**"]
enabled = false           # Disable analysis for vendor files

[overrides."src/legacy/**"]
rules.high_fanout.min_fanout = 40
rules.high_fanout.relative_to_p90 = false

# ============================================================
# [services] — Cross-service dependency tracking
# ============================================================
[services."billing"]
root = "services/billing"
lang = "python"
graphql_schemas = ["services/billing/schema.graphql"]
openapi_specs = ["services/billing/openapi.yaml"]
base_urls = ["https://billing.internal"]
```

## Section Details

### `[defaults]`

General operational settings. All fields are optional; unset fields use built-in defaults.

| Field | Type | Default | Description |
|-------|------|---------|-------------|
| `lang` | string | auto-detect | Language to analyze |
| `format` | string | `"json"` | Output format |
| `quiet` | bool | `false` | Suppress progress output |
| `top` | integer | none (show all) | Limit hotspot count |
| `include_tests` | bool | `false` | Include test files |
| `no_insights` | bool | `false` | Suppress insights |

### `[targeting]`

File inclusion and exclusion patterns (glob syntax).

| Field | Type | Default | Description |
|-------|------|---------|-------------|
| `include` | string array | `[]` (include all) | Only analyze matching paths |
| `exclude` | string array | `[]` | Skip matching paths |

### `[rules.*]`

See [Insights](../insights/README.md) for detailed documentation of each rule.

### `[fail_on]`

| Field | Type | Default | Description |
|-------|------|---------|-------------|
| `conditions` | string array | `[]` | [Fail-on conditions](../ci-integration/fail-on.md) for `diff` |

### `[overrides]`

See [Per-Path Overrides](./overrides.md).

### `[services]`

Used by `untangle service-graph` to map service boundaries and API contracts.

Each entry is keyed by service name:

```toml
[services."billing"]
root = "services/billing"
lang = "python" # optional
graphql_schemas = ["services/billing/schema.graphql"]
openapi_specs = ["services/billing/openapi.yaml"]
base_urls = ["https://billing.internal"]
```

| Field | Type | Description |
|-------|------|-------------|
| `root` | string | Service root directory (relative to project root) |
| `lang` | string | Optional language override (`python`, `ruby`, `go`, `rust`) |
| `graphql_schemas` | string array | GraphQL schema file paths |
| `openapi_specs` | string array | OpenAPI spec file paths |
| `base_urls` | string array | Base URLs used to match REST client calls |

## Backward Compatibility

The old configuration format with `[thresholds]` and `[defaults].exclude` is still supported:

```toml
# Old format (still works)
[defaults]
lang = "python"
exclude = ["vendor/**"]   # Migrated to [targeting].exclude

[thresholds]
max_fanout = 15           # Migrated to [rules.high_fanout].min_fanout
max_scc_size = 5          # Migrated to [rules.circular_dependency].warning_min_size
```

Migration happens automatically at load time. New fields take precedence if both old and new are present.
