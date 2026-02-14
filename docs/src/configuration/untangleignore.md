# .untangleignore

The `.untangleignore` file provides a gitignore-style way to exclude files from analysis.

## Location

Place `.untangleignore` in your project root (or any ancestor directory). Untangle walks up from the working directory to find it.

## Syntax

```
# Comments start with #
vendor/**
node_modules/**

# Generated files
*.generated.go
*.pb.go

# Build output
build/
dist/
```

### Rules

- Blank lines are ignored
- Lines starting with `#` are comments
- All other lines are glob patterns added to the exclusion list
- Patterns use the same glob syntax as `[targeting].exclude`

## Relationship to Config File

Patterns from `.untangleignore` are **merged** with `[targeting].exclude` from the config file. Both sources contribute to the final exclusion list.

```toml
# .untangle.toml
[targeting]
exclude = ["**/test/**"]
```

```
# .untangleignore
vendor/**
*.generated.go
```

Effective exclusions: `["**/test/**", "vendor/**", "*.generated.go"]`

## When to Use

Use `.untangleignore` when you want to:

- Keep exclusion patterns separate from the config file
- Share exclusion patterns across teams without modifying `.untangle.toml`
- Maintain a file similar to `.gitignore` that developers are already familiar with
