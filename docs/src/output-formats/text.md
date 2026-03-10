# Text Format

The text format produces a human-readable report for terminal use.

Text is available on:

- `untangle analyze report`
- `untangle diff`
- `untangle quality functions`
- `untangle quality project`
- `untangle service-graph`

## Usage

```bash
untangle analyze report ./src --lang python --format text
untangle analyze report ./src --lang python --format text --top 10
untangle diff --base origin/main --head HEAD --format text
untangle quality functions . --metric crap --coverage lcov.info --format text
```
