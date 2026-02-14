# Language Support

Untangle parses source files using [tree-sitter](https://tree-sitter.github.io/tree-sitter/) grammars, extracting import statements and resolving them to project-internal modules.

## Language Comparison

| Feature | Python | Ruby | Go | Rust |
|---------|--------|------|----|------|
| Granularity | File/module | File | Package | Module |
| Import syntax | `import`, `from...import` | `require`, `require_relative` | `import "path"` | `use crate::...` |
| Manifest file | - | - | `go.mod` | `Cargo.toml` |
| Stdlib filtering | N/A | N/A | `exclude_stdlib` (default: on) | N/A |
| Relative imports | `resolve_relative` | `require_relative` | N/A | `use self::`, `use super::` |
| Config section | `[python]` | `[ruby]` | `[go]` | - |

## Language Detection

If `--lang` is not specified and no `lang` is set in config, untangle auto-detects the language by examining file extensions in the target directory.

## File Extensions

| Language | Extensions |
|----------|-----------|
| Python | `.py` |
| Ruby | `.rb` |
| Go | `.go` |
| Rust | `.rs` |
