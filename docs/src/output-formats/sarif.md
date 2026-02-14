# SARIF Format

Untangle can output results in [SARIF 2.1.0](https://sarifweb.azurewebsites.net/) (Static Analysis Results Interchange Format), designed for integration with GitHub Code Scanning and other SARIF-consuming tools.

## Usage

```bash
untangle analyze ./src --lang python --format sarif > results.sarif
```

## Rules

The SARIF output includes two rule definitions:

| Rule ID | Name | Description |
|---------|------|-------------|
| `untangle/high-fanout` | HighFanOut | Module has excessive fan-out (too many dependencies) |
| `untangle/circular-dependency` | CircularDependency | Module is part of a circular dependency |

## Results

### High Fan-out

A result is generated for each module whose fan-out exceeds the threshold (default: 10, configurable via `--threshold-fanout`):

```json
{
  "ruleId": "untangle/high-fanout",
  "level": "warning",
  "message": {
    "text": "Module 'src/core/engine' has fan-out of 23 (threshold: 15)"
  },
  "locations": [{
    "physicalLocation": {
      "artifactLocation": { "uri": "src/core/engine.py" }
    }
  }]
}
```

### Circular Dependency

A result is generated for each module that belongs to a non-trivial strongly connected component:

```json
{
  "ruleId": "untangle/circular-dependency",
  "level": "warning",
  "message": {
    "text": "Module 'src/api/auth' is part of a circular dependency (SCC #0, 12 members)"
  },
  "locations": [{
    "physicalLocation": {
      "artifactLocation": { "uri": "src/api/auth.py" }
    }
  }]
}
```

## GitHub Code Scanning

See [SARIF Upload](../ci-integration/sarif-upload.md) for how to upload SARIF results to GitHub Code Scanning.
