# Ruby

Untangle parses Ruby files at file level, extracting `require` and `require_relative` statements.

## What Gets Parsed

```ruby
require 'json'                     # External — skipped
require 'myapp/models/user'        # Resolved via load_path
require_relative '../helpers/auth' # Resolved relative to current file
```

## Import Resolution

1. `require_relative` paths are resolved relative to the current file
2. `require` paths are resolved against the configured `load_path` directories
3. Imports that don't match a project file are classified as external

## Configuration

```toml
[ruby]
zeitwerk = false              # Default: false
load_path = ["lib", "app"]   # Default: ["lib", "app"]
```

| Field | Type | Default | Description |
|-------|------|---------|-------------|
| `zeitwerk` | bool | `false` | Use Zeitwerk autoload conventions |
| `load_path` | string array | `["lib", "app"]` | Directories to search for `require` targets |

## Load Path

The `load_path` setting mirrors Ruby's `$LOAD_PATH`. When resolving `require 'myapp/models/user'`, untangle looks for:

1. `lib/myapp/models/user.rb`
2. `app/myapp/models/user.rb`

The first match wins.

## What Gets Skipped

- Gems and standard library requires that don't match project files
- Dynamic requires (`require some_variable`)
- Conditional requires inside `if` blocks

## Example

For a Rails-style project:

```
app/
├── models/
│   └── user.rb           # require_relative '../services/auth'
├── services/
│   └── auth.rb           # require 'app/models/user'
lib/
└── utils/
    └── helpers.rb
```

With `load_path = ["app", "lib"]`, untangle resolves:
- `app/models/user` -> `app/services/auth` (via `require_relative`)
- `app/services/auth` -> `app/models/user` (via `require` + load_path)
