# Python

Untangle parses Python files at module (file) level, extracting `import` and `from ... import` statements.

## What Gets Parsed

```python
import os                          # External — skipped
import mypackage.module            # Resolved to mypackage/module.py
from mypackage import module       # Resolved to mypackage/module.py
from . import sibling              # Relative import (if resolve_relative=true)
from ..parent import child         # Relative import
```

## Import Resolution

1. Each `import` or `from` statement is extracted via tree-sitter
2. The import path is matched against files in the project directory
3. Imports that don't resolve to a project file are classified as external and skipped
4. Relative imports (starting with `.`) are resolved relative to the importing file's directory

## Configuration

```toml
[python]
resolve_relative = true   # Default: true
```

| Field | Type | Default | Description |
|-------|------|---------|-------------|
| `resolve_relative` | bool | `true` | Resolve relative imports (`from . import ...`) |

When `resolve_relative` is `false`, relative imports are treated as unresolvable.

## What Gets Skipped

- Standard library imports (`os`, `sys`, `json`, etc.)
- Third-party packages not found in the project tree
- Dynamic imports (`importlib.import_module(...)`)
- Conditional imports inside `try/except`

## Example

For a project structure:

```
src/
├── core/
│   ├── __init__.py
│   └── engine.py      # from src.api import handler
├── api/
│   ├── __init__.py
│   └── handler.py     # from src.core import engine
```

Untangle produces the edges:
- `src/core/engine` -> `src/api/handler`
- `src/api/handler` -> `src/core/engine`

And detects an SCC (circular dependency) between these two modules.
