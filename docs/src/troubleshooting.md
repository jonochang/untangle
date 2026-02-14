# Troubleshooting

## Common Issues

### "No files found" error

```
Error: No files found at path: ./src
```

**Causes:**
- The path doesn't exist or is empty
- Language auto-detection failed (no files with matching extensions found)
- All files were excluded by `[targeting].exclude` or `.untangleignore`

**Solutions:**
- Verify the path exists: `ls ./src`
- Specify the language explicitly: `--lang python`
- Check your exclusion patterns: `untangle config show`

### Language auto-detection picks the wrong language

If your project has mixed languages, auto-detection may pick the wrong one.

**Solution:** Always specify `--lang` explicitly, or set it in `.untangle.toml`:

```toml
[defaults]
lang = "python"
```

### "Could not read config" error

```
Error: Invalid project config: ...
```

**Causes:**
- Syntax error in `.untangle.toml`
- Invalid TOML format

**Solutions:**
- Validate your TOML: use a TOML linter or `untangle config show` to identify the error
- Check for common TOML issues (missing quotes, incorrect table syntax)

### High number of unresolved imports

If the report shows many unresolved imports:

**For Python:**
- Third-party packages are expected to be unresolvable — this is normal
- Check that your project structure matches the import paths

**For Go:**
- Ensure `go.mod` exists and the module path is correct
- External dependencies are expected to be unresolvable

**For Ruby:**
- Check that `load_path` is configured correctly in `[ruby]`
- Gems are expected to be unresolvable

**For Rust:**
- Ensure `Cargo.toml` is present
- External crate imports are expected to be unresolvable

### Git diff fails with "reference not found"

```
Error: Could not find reference: origin/main
```

**Solutions:**
- Ensure you've fetched the base ref: `git fetch origin main`
- In CI, use `fetch-depth: 0` to get full history
- Use the correct ref name (e.g., `origin/main` vs `origin/master`)

### Analysis is slow

For large projects with 10,000+ files:

- Untangle uses Rayon for parallel parsing — it should process thousands of files per second
- Use `--exclude` to skip vendored code, generated files, and test directories
- Use `.untangleignore` for persistent exclusions
- Ensure you're running a release build (`cargo build --release`)

### Empty graph (0 edges)

If analysis reports 0 edges:

- Check that the language is correct (`--lang`)
- Verify that the project has internal imports (not just external/stdlib)
- Check that the path points to the right directory (not too narrow or too broad)
- Use `--format json` to see `unresolved_imports` count

## Getting Help

- [GitHub Issues](https://github.com/user/untangle/issues) — report bugs or request features
- Run `untangle config show` to debug configuration issues
- Run `untangle config explain <rule>` to understand rule thresholds
