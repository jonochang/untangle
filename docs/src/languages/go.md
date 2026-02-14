# Go

Untangle parses Go files at package level, extracting `import` declarations and resolving them using `go.mod`.

## What Gets Parsed

```go
import (
    "fmt"                                    // stdlib — skipped (default)
    "github.com/user/myproject/pkg/handler"  // Resolved to pkg/handler/
    "github.com/other/library"               // External — skipped
)
```

## Import Resolution

1. Import paths are extracted from `import` declarations via tree-sitter
2. The `go.mod` file is read to determine the module path (e.g., `github.com/user/myproject`)
3. Imports prefixed with the module path are resolved to directories within the project
4. Standard library imports (no `.` in path) are excluded by default
5. External dependencies are skipped

## Configuration

```toml
[go]
exclude_stdlib = true   # Default: true
```

| Field | Type | Default | Description |
|-------|------|---------|-------------|
| `exclude_stdlib` | bool | `true` | Exclude Go standard library imports from the graph |

## Package-Level Granularity

Go analysis operates at the **package** (directory) level, not the file level. All `.go` files in a directory are treated as part of the same package. The graph nodes are package paths relative to the project root.

## go.mod

Untangle reads `go.mod` to determine the module path. If `go.mod` is not found, import resolution falls back to directory matching.

Example `go.mod`:

```
module github.com/user/myproject

go 1.21
```

With this module path, `import "github.com/user/myproject/pkg/handler"` resolves to `pkg/handler/`.

## Test Files

Go test files (`*_test.go`) are excluded by default. Use `--include-tests` or set `include_tests = true` in config to include them.

## What Gets Skipped

- Standard library imports (when `exclude_stdlib = true`)
- External dependency imports (anything not under the module path)
- CGo imports (`import "C"`)

## Example

```
cmd/
└── server/
    └── main.go          # import "github.com/user/myproject/internal/api"
internal/
├── api/
│   └── handler.go       # import "github.com/user/myproject/internal/db"
└── db/
    └── models.go
pkg/
└── utils/
    └── helpers.go
```

Graph nodes: `cmd/server`, `internal/api`, `internal/db`, `pkg/utils`
