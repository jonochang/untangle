# Changelog

All notable changes to this project will be documented in this file.

## [0.3.2] - 2026-03-06

### Fixed

- Pin Nix package source to an immutable commit instead of a mutable release tag archive.
- Update Nix source hash to match the pinned source and avoid fixed-output hash mismatch errors in consumers.
- Replace incompatible package test skip flags with `cargoTestFlags = ["--bins"]` so Nix check phase passes reliably.

## [0.3.0] - 2026-03-05

### Added

- Function-level code quality metrics with CRAP scoring, LCOV coverage parsing, and JSON/text reporting.
- Overall quality report combining Untangle structural metrics with CRAP, including Untangle hotspots.
- Cucumber BDD test suite covering analyze, graph, diff, quality, and config commands.

## [0.2.0] - 2026-02-16

### Added

- **Multi-language monorepo support**: Auto-detect all languages (Go, Python, Ruby, Rust) when `--lang` is omitted. Per-language stats reported in `languages` array.
- **Cross-service API dependency tracking**: New `service-graph` command detects GraphQL and REST/OpenAPI dependencies between services declared in `.untangle.toml`.
- **Per-language resolution coverage**: `imports_resolved` and `imports_unresolved` fields in JSON output per language, making it easy to see parser coverage (e.g. "Ruby: 22/4800 imports resolved").
- **Nested Go module detection**: Monorepos with multiple `go.mod` files (e.g. `web/golang/go.mod`, `api/go.mod`) are automatically discovered. Each Go file resolves imports against its nearest module root.
- **Zeitwerk Ruby resolution**: When `[ruby] zeitwerk = true` is set in config, Ruby constant references (`User`, `Admin::User`) are resolved to files via `CamelCase -> snake_case` convention through configured load paths. Includes stdlib constant exclusion list.
- SARIF output format for `analyze` command.
- DOT output with language-colored nodes for multi-language graphs.
- `config show` and `config explain` subcommands for configuration introspection.
- `.untangleignore` file support (gitignore-style patterns).
- Per-path rule overrides via `[overrides."glob"]` in config.

### Fixed

- 16 issues across licensing, diff logic, parsers, and configuration (see 09186a1).
- Go test file exclusion in multi-language mode.
- Rust scoped use list parsing skips anonymous punctuation nodes.

## [0.1.0] - 2025-12-01

### Added

- Initial release.
- Go, Python, Ruby, and Rust language support via tree-sitter.
- `analyze` command: fan-out, fan-in, Shannon entropy, SCC detection.
- `diff` command: compare dependency graphs between git revisions with CI policy gating.
- `graph` command: export dependency graph as DOT or JSON.
- Configurable via `.untangle.toml` with layered resolution (user, project, env, CLI).
- Parallel file parsing with rayon.
- Progress bars with indicatif.
- Insights/suggestions engine with configurable rules.
