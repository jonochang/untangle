# SARIF Format

Untangle can output [SARIF 2.1.0](https://sarifweb.azurewebsites.net/) for code-scanning workflows.

## Usage

```bash
untangle analyze report ./src --lang python --format sarif > results.sarif
```

SARIF is supported only for `analyze report`.

## Rules

The SARIF output includes:

- `untangle/high-fanout`
- `untangle/circular-dependency`

See [SARIF Upload](../ci-integration/sarif-upload.md) for GitHub Code Scanning integration.
