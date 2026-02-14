# Rust

Untangle parses Rust files at module level, extracting `use` statements and resolving them using `Cargo.toml`.

## What Gets Parsed

```rust
use std::collections::HashMap;        // stdlib — skipped
use crate::config::schema::FileConfig; // Resolved to src/config/schema.rs
use super::common::ImportConfidence;   // Resolved relative to parent module
use self::submodule::helper;           // Resolved within current module
```

## Import Resolution

1. `use` statements are extracted via tree-sitter, including scoped imports (`use crate::{a, b}`)
2. `Cargo.toml` is read to determine the crate name
3. `use crate::` paths are resolved relative to `src/`
4. `use super::` and `use self::` are resolved relative to the importing file
5. External crate imports are skipped

## Crate Name Detection

Untangle reads `Cargo.toml` to find the crate name:

```toml
[package]
name = "untangle"
```

This is used to resolve `use untangle::...` as equivalent to `use crate::...`.

## Scoped Imports

Untangle handles all Rust import patterns:

```rust
// Simple path
use crate::config::schema;

// Scoped/grouped imports
use crate::config::{schema, resolve, overrides};

// Nested scoped imports
use crate::parse::{
    go::GoFrontend,
    python::PythonFrontend,
};

// Glob imports
use crate::config::*;

// Self imports
use crate::config::schema::{self, FileConfig};
```

## What Gets Skipped

- Standard library imports (`use std::...`)
- External crate imports (`use serde::...`, `use clap::...`)
- Macro imports (`use crate::my_macro!`)

## Module Resolution

Rust modules are resolved using standard Rust module conventions:

| Use Path | Resolves To |
|----------|-------------|
| `crate::config::schema` | `src/config/schema.rs` |
| `crate::config` | `src/config/mod.rs` or `src/config.rs` |
| `super::common` | `../common.rs` or `../common/mod.rs` |
| `self::helper` | `./helper.rs` or `./helper/mod.rs` |

## Example

```
src/
├── main.rs
├── lib.rs
├── config/
│   ├── mod.rs           # use crate::metrics::summary;
│   ├── schema.rs        # (no internal imports)
│   └── resolve.rs       # use crate::config::schema;
└── metrics/
    ├── mod.rs
    └── summary.rs       # use crate::config;
```

Graph edges:
- `src/config` -> `src/metrics/summary`
- `src/config/resolve` -> `src/config/schema`
- `src/metrics/summary` -> `src/config`
