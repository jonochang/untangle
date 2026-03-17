# SCRAP-like Spec Quality Plan for Untangle

## Summary

Add a new `untangle quality specs [PATH]` command that analyzes test/spec code
quality across Python, Ruby, Go, and Rust, then reports where test structure is
weak and how to improve it. The feature is guidance-first, not autofix-first,
and includes a baseline workflow (`--write-baseline`, `--compare <FILE>`) so
teams can judge whether a spec refactor made things better or worse.

The design should mirror `SCRAP`'s value, not its Speclj-specific
implementation: detect oversized or logic-heavy tests, weak assertions,
repeated setup, heavy mocking, and table-driven candidates; score them at
test/block/file levels; and emit `stable` / `local` / `split` guidance with
explicit locations and evidence.

## Public Surface

- Add a new subcommand under `quality`:
  - `untangle quality specs [PATH]`
- Support `--lang`, `--format text|json`, `--top <N>`, `--quiet`,
  `--write-baseline`, and `--compare <FILE>`.
- Add a new config section for this command with at least:
  - default `format`
  - default `top`
  - score thresholds for `stable` / `local` / `split`
- Output should be separate from `quality report` in v1. Do not embed spec
  quality into the unified report yet.
- JSON should use a dedicated envelope, for example `kind = "quality.specs"`,
  with its own schema version.
- Text output should follow the same "guidance first, evidence second" pattern
  used elsewhere in `untangle`.

## Implementation Changes

### 1. New spec-quality subsystem

- Add a new module tree such as `src/spec_quality/` to keep this feature
  isolated from production-code quality logic.
- Split the subsystem into:
  - file discovery and test-target filtering
  - per-language test/spec extraction
  - shared metrics/scoring
  - guidance/comparison
  - text/json rendering

### 2. Cross-language test discovery

- Analyze only test/spec files by default.
- Use conventional file/path patterns per language:
  - Python: `tests/**`, `test_*.py`, `*_test.py`
  - Ruby: `spec/**`, `test/**`, `*_spec.rb`, `test_*.rb`
  - Go: `*_test.go`
  - Rust: `tests/**`, plus in-crate modules/functions marked with test
    attributes
- `--lang` should narrow the analyzer; otherwise reuse current language
  auto-detection.

### 3. Language-specific extraction with shared scoring

- Use a hybrid AST + heuristics approach:
  - language-specific parsers locate test files, test blocks, and test cases
  - shared scoring computes comparable structural metrics
- v1 framework support:
  - Python: pytest-style test functions/methods and `unittest.TestCase` methods
  - Ruby: RSpec examples/contexts plus Minitest-style `test_` methods
  - Go: `testing` package `Test*` functions
  - Rust: `#[test]` and common test-like attribute macros that clearly mark
    tests
- For each test/example capture:
  - path, name, enclosing block/context path, line span
  - line count
  - assertion count
  - branch count
  - setup depth
  - mock/stub/redef count
  - local helper indirection count
  - whether it is already table-driven
  - smell labels
  - score
- For each file/block compute:
  - example count
  - average/max score
  - branching example count
  - low-assertion / zero-assertion counts
  - mocking-heavy count
  - duplication/repetition indicators
  - worst examples

### 4. Scoring and guidance model

- Keep the v1 score intentionally simpler than full `SCRAP`, but preserve the
  same behavior:
  - tests get more expensive as branching, setup depth, helper indirection, and
    mocking increase
  - direct smell penalties apply for weak assertions, oversized tests, multiple
    phases, heavy mocking, and repeated scaffolding
- Guidance should be emitted at file level with:
  - `pressure`
  - `remediation_mode` = `stable | local | split`
  - `ai_actionability`
  - `why`
  - `where`
  - `how`
- Recommendation rules should include at minimum:
  - strengthen assertions before structural cleanup
  - split oversized tests
  - reduce mocking/stubbing
  - extract repeated setup only when harmful duplication dominates
  - convert repeated low-complexity cases into table-driven tests
  - split a spec/test file when pressure is spread across multiple hotspots
- Guidance must include concrete locations and evidence, not just labels.

### 5. Baseline and comparison workflow

- Add `--write-baseline` to write a deterministic JSON baseline to
  `target/untangle/specs.json` by default.
- Add `--compare <FILE>` to compare the current report against a prior baseline.
- Comparison should be top-level and per-file, with verdicts:
  - `improved`
  - `worse`
  - `mixed`
  - `unchanged`
- Comparison should key off score delta plus the highest-signal
  regressions/improvements:
  - average/max score
  - harmful duplication/repetition
  - weak-assertion counts
  - mocking-heavy counts
- If comparison says `worse`, text output should say so explicitly and recommend
  reverting or simplifying the attempted refactor.

### 6. Output model

- Introduce report types roughly shaped as:
  - `SpecQualityReport`
  - `SpecFileReport`
  - `SpecBlockReport`
  - `SpecCaseReport`
  - `SpecGuidance`
  - `SpecComparison`
- Text output should be optimized for "what should I refactor, where, and how".
- JSON output should preserve all metrics and locations so it can drive future
  assistant workflows.
- Do not add code-rewrite or autofix output in v1.

## Test Plan

- CLI coverage:
  - `quality specs` works in text and json
  - `--write-baseline` writes the default sidecar baseline
  - `--compare` attaches comparison verdicts
- Discovery coverage:
  - per-language test/spec file selection matches conventions
  - non-test files are ignored by default
- Extraction coverage:
  - Python pytest and unittest
  - Ruby RSpec and Minitest
  - Go `Test*`
  - Rust `#[test]`
- Scoring/guidance coverage:
  - oversized test
  - logic-heavy test
  - low/zero-assertion test
  - mocking-heavy test
  - repeated setup / table-driven candidate
  - `stable`, `local`, and `split` guidance verdicts
- Comparison coverage:
  - improved / worse / mixed / unchanged file verdicts
  - top-level compare summary reflects file-level results
  - baseline output is deterministic and stable
- Integration coverage:
  - mixed-language repo with test files in more than one language
  - locations/evidence show file paths and line ranges in both text and json

## Assumptions and Defaults

- v1 is a dedicated `quality specs` surface, not part of `quality report`.
- v1 supports Python, Ruby, Go, and Rust, but only for common test styles;
  unusual frameworks are ignored rather than guessed.
- v1 is guidance-oriented only. No autofix or code mutation output is planned.
- Baseline comparison is file-based, not git-ref-based, in v1.
- Default output should be human-oriented (`text`) because the primary value is
  remediation guidance; JSON remains first-class for tooling.
- The feature should reuse existing parser infrastructure where possible, but it
  is allowed to add test-specific extraction logic per language when needed.
