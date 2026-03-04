# CRAP Metric Implementation Plan for Untangle

## Overview

The **CRAP (Change Risk Anti-Pattern)** metric identifies functions that are simultaneously
complex and under-tested. It is the highest-signal indicator of risky code to change.

### Formula

```
CRAP(fn) = CC² × (1 - coverage)³ + CC
```

- **CC** = cyclomatic complexity (number of linearly-independent paths)
- **coverage** = fraction of instrumented lines hit by tests (0.0–1.0)

**Behaviour at extremes:**
- 100% coverage → `CRAP = CC` (score collapses to complexity alone)
- 0% coverage   → `CRAP = CC² + CC` (maximum penalty)

**Risk bands** (from the original paper by Tornhill & Bowes):
| CRAP score | Risk |
|---|---|
| 1–5 | Low – clean |
| 5–30 | Moderate – refactor or add tests |
| 30+ | High – complex and untested |

---

## Coverage Input Format

Untangle will consume **LCOV** (`.lcov` / `lcov.info`) files. LCOV is the de-facto
standard for Rust (`cargo llvm-cov --lcov`), Python (`coverage.py --lcov`),
Go (`go test -coverprofile` + `gcov2lcov`), and Ruby (`simplecov-lcov`).

Relevant LCOV fields:
```
SF:<source-file-path>
DA:<line>,<hit-count>   # one entry per instrumented line
end_of_record
```

Coverage for a function = `covered_lines / instrumented_lines` over its line span.
Lines with no `DA` entry (blank lines, comments) are excluded from the denominator.

---

## New Modules

```
src/
├── complexity/
│   ├── mod.rs      # ComplexityFrontend trait + FunctionInfo type
│   ├── rust.rs     # Rust CC (tree-sitter)
│   ├── python.rs   # Python CC (tree-sitter)
│   ├── go.rs       # Go CC (tree-sitter)
│   └── ruby.rs     # Ruby CC (tree-sitter)
├── coverage/
│   ├── mod.rs      # CoverageMap type + coverage_for_range helper
│   └── lcov.rs     # LCOV file parser
├── metrics/
│   └── crap.rs     # CRAP formula, CrapResult, risk_band helper
└── cli/
    └── crap.rs     # `untangle crap` subcommand
```

---

## Key Types

### `complexity/mod.rs`

```rust
/// One function extracted from a source file.
pub struct FunctionInfo {
    pub name: String,
    pub start_line: usize,   // 1-indexed, inclusive
    pub end_line: usize,     // 1-indexed, inclusive
    pub cyclomatic_complexity: usize,
}

/// Implemented per language.
pub trait ComplexityFrontend {
    fn language(&self) -> tree_sitter::Language;
    /// Return all top-level and nested function/method definitions.
    fn extract_functions(&self, source: &[u8], file_path: &Path) -> Vec<FunctionInfo>;
}
```

### `coverage/mod.rs`

```rust
/// file-path → (line → hit-count)
pub type CoverageMap = HashMap<PathBuf, HashMap<usize, u64>>;

/// Fraction of instrumented lines in [start, end] that were hit at least once.
/// Returns None when no instrumented lines exist (macro-generated code, etc.).
pub fn coverage_for_range(
    file_coverage: &HashMap<usize, u64>,
    start_line: usize,
    end_line: usize,
) -> Option<f64>;
```

### `metrics/crap.rs`

```rust
pub struct CrapResult {
    pub file: PathBuf,
    pub function: String,
    pub start_line: usize,
    pub cyclomatic_complexity: usize,
    pub coverage_pct: f64,   // 0.0–100.0; NaN encodes "no data"
    pub crap_score: f64,
}

pub fn crap_score(cc: f64, coverage: f64) -> f64 {
    cc * cc * (1.0 - coverage).powi(3) + cc
}

pub fn risk_band(score: f64) -> &'static str { /* Low / Moderate / High */ }
```

---

## Cyclomatic Complexity — Decision Points per Language

CC starts at **1** for every function, then +1 per decision point.

### Rust

| Node type | Rationale |
|---|---|
| `if_expression` | branch |
| `while_expression` | loop |
| `while_let_expression` | loop + pattern |
| `loop_expression` | unconditional loop still raises path count |
| `for_expression` | iteration |
| `match_arm` | each arm is a branch (not the match expression itself) |
| `binary_expression` with `&&` or `\|\|` | short-circuit creates new path |
| `?` operator (`try_expression`) | early-return path |

### Python

| Node type | Rationale |
|---|---|
| `if_statement` | branch |
| `elif_clause` | additional branch |
| `while_statement` | loop |
| `for_statement` | iteration |
| `except_clause` | exception path |
| `boolean_operator` (`and` / `or`) | short-circuit |
| `conditional_expression` | ternary |

### Go

| Node type | Rationale |
|---|---|
| `if_statement` | branch |
| `for_statement` | all loop forms |
| `case_clause` | switch arm |
| `type_case_clause` | type-switch arm |
| `communication_case` | select arm |
| `binary_expression` with `&&` or `\|\|` | short-circuit |

### Ruby

| Node type | Rationale |
|---|---|
| `if` / `unless` | branch |
| `elsif` | additional branch |
| `while` / `until` | loop |
| `for` | iteration |
| `when` | case arm |
| `rescue` | exception path |
| `binary` with `&&` / `\|\|` / `and` / `or` | short-circuit |
| `conditional` (ternary `?:`) | inline branch |

---

## CLI Interface

New subcommand added to the dispatcher in `cli/mod.rs`:

```
untangle crap [OPTIONS] <PATH>

Arguments:
  <PATH>      Root directory (or single file) to analyse

Options:
  --coverage <FILE>     LCOV coverage file [required]
  --language <LANG>     rust | python | go | ruby [required]
  --min-cc <N>          Only report functions with CC >= N [default: 2]
  --min-crap <N>        Only report functions with CRAP >= N [default: 0]
  --format <FMT>        text | json [default: text]
  --top <N>             Show only top N results [default: unlimited]
```

### Example invocation (Rust project)

```bash
cargo llvm-cov --lcov --output-path lcov.info
untangle crap --coverage lcov.info --language rust ./src
```

### Text output (sorted descending by CRAP)

```
CRAP Report
===========
Function                           File                         CC    Cov%   CRAP   Risk
───────────────────────────────────────────────────────────────────────────────────────
parse_complex_expr                 src/parse/python.rs:142      14    12.3%  269.4  High
resolve_imports                    src/parse/resolver.rs:88     10    34.0%   56.3  High
build_graph                        src/graph/builder.rs:55       8    67.0%   14.2  Moderate
```

### JSON output (subset)

```json
{
  "language": "rust",
  "coverage_file": "lcov.info",
  "results": [
    {
      "file": "src/parse/python.rs",
      "function": "parse_complex_expr",
      "start_line": 142,
      "cyclomatic_complexity": 14,
      "coverage_pct": 12.3,
      "crap_score": 269.4,
      "risk": "High"
    }
  ]
}
```

---

## Implementation Steps (ordered)

1. **`coverage/lcov.rs`** — Parse LCOV into `CoverageMap`. No new dependencies;
   LCOV is plain text. Include `coverage_for_range` helper.

2. **`complexity/mod.rs`** — Define `ComplexityFrontend` trait and `FunctionInfo`
   struct. Add factory function `complexity_frontend_for(lang) -> Box<dyn ComplexityFrontend>`.

3. **`complexity/rust.rs`** — Implement CC for Rust using tree-sitter.
   Reuse `tree-sitter-rust` already in `Cargo.toml` (via the existing `parse::rust`
   module). Write a tree-sitter query that walks the AST of each `function_item`,
   counting the decision nodes listed above.

4. **`complexity/python.rs`** — Same pattern for `tree-sitter-python`.

5. **`complexity/go.rs`** — Same pattern for `tree-sitter-go`.

6. **`complexity/ruby.rs`** — Same pattern for `tree-sitter-ruby`.

7. **`metrics/crap.rs`** — Implement `crap_score`, `CrapResult`, and `risk_band`.

8. **`cli/crap.rs`** — Wire together:
   - Walk files via existing `walk.rs`
   - Load coverage via `coverage::lcov`
   - Extract functions via `complexity` frontend
   - Compute CRAP per function via `metrics::crap`
   - Filter by `--min-cc` / `--min-crap`
   - Sort descending by CRAP score
   - Emit text or JSON output

9. **`cli/mod.rs`** — Add `Crap(CrapArgs)` variant and dispatch branch.

10. **`main.rs`** — No changes required; dispatch goes through `cli/mod.rs`.

---

## Notes and Constraints

- **Complexity scope**: Only top-level functions are reported. Closures/lambdas
  increment the enclosing function's CC (they share the same control flow scope
  for risk purposes), consistent with the original CRAP paper.
- **No new Cargo dependencies**: tree-sitter language crates are already present;
  LCOV parsing uses std I/O only.
- **Uncovered functions**: If a function has no LCOV entries (e.g. dead code never
  compiled into the test binary), `coverage = 0%` is assumed, yielding maximum CRAP.
  This is reported explicitly in output.
- **Inlined / generated code**: Functions whose `start_line == end_line` (single-
  line bodies, common for derives) are excluded unless `--min-cc 1` is passed.
- **Match arms in Rust**: Wildcard arms (`_ => ...`) count as +1 because they
  represent a distinct reachable path.
