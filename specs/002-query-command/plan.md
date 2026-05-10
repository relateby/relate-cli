# Implementation Plan: relate query — Single Query Execution

**Branch**: `002-query-command` | **Date**: 2026-05-10 | **Spec**: [spec.md](spec.md)
**Input**: Feature specification from `specs/002-query-command/spec.md`

## Summary

Add `relate query` — a subcommand that executes a single Cypher statement (from an
inline `-e` expression or a `.cypher` file) against Neo4j via Bolt, with a preflight
pipeline that lints, classifies read/write, and validates parameters before opening
any connection. Write statements require `--write`. Results are rendered as a Unicode
table (human) or JSON array (machine). Multi-statement files execute in order; the
first error aborts remaining statements.

## Technical Context

**Language/Version**: Rust 1.85.0 (MSRV as pinned in `Cargo.toml`)
**Primary Dependencies**: clap 4.6 (derive), tokio 1 (full), neo4rs 0.9.0-rc.9,
  tree-sitter-cypher 0.2 (via cypher-data), cypher-data 0.2.3, ariadne 0.6,
  serde/serde_json 1, comfy-table 7 (new — table output)
**Storage**: N/A — no local storage; results to stdout
**Testing**: `cargo test` — unit tests for preflight stages; integration tests via
  `assert_cmd` against a live or mocked Neo4j connection
**Target Platform**: macOS, Linux, Windows (single binary CLI)
**Project Type**: CLI tool (single crate, no workspace)
**Performance Goals**: Preflight completes in < 1 second for files with up to 100
  statements; query round-trip dominated by Neo4j latency
**Constraints**: No interactive prompts; credentials from flags/env only; no daemon;
  single binary; async for Bolt I/O, sync for preflight

## Constitution Check

*GATE: All four principles verified. No violations.*

| Principle | Status | Notes |
|-----------|--------|-------|
| I. CLI-Friendly | ✅ Pass | Results to stdout, errors/diagnostics to stderr; exit codes 0/1/2/≥3; no interactive prompts; all config from flags/env |
| II. Human-Readable | ✅ Pass | Unicode table output via `comfy-table`; ariadne-style lint diagnostics; error messages include actionable hints |
| III. Agent-Friendly | ✅ Pass | `--json` flag with stable schema (see `contracts/query-cli.md`); candidate for `relate mcp` exposure in a later milestone |
| IV. Self-Contained Help | ✅ Pass | `--help` includes purpose, all flags with defaults, two usage examples, preflight note, and write-flag note |

## Project Structure

### Documentation (this feature)

```text
specs/002-query-command/
├── plan.md              # This file
├── research.md          # Phase 0 — technology decisions
├── data-model.md        # Phase 1 — internal types
├── contracts/
│   └── query-cli.md     # Phase 1 — CLI contract (args, exit codes, output schema)
└── tasks.md             # Phase 2 output (/speckit-tasks — not yet created)
```

### Source Code

```text
src/
├── main.rs              # Add Commands::Query arm (exit code handling)
├── cli.rs               # Add QueryArgs struct; add Query variant to Commands
└── commands/
    ├── mod.rs           # Add pub mod query
    └── query.rs         # New — full query command implementation
```

New dependency in `Cargo.toml`:
```toml
comfy-table = "7"
```

## Phase 0: Research

See [research.md](research.md) — all unknowns resolved. Summary:

- **neo4rs API**: `Graph::execute(query(...).param(...))` → `RowStream`; see research Decision 1.
- **Multi-statement splitting**: tree-sitter root → iterate `statement` children; see Decision 2.
- **Write classification**: AST walk for write clause node kinds; see Decision 3.
- **Table output**: `comfy-table` crate with `UTF8_FULL` preset; see Decision 4.
- **Async**: `query::run` is `async`; preflight is sync within; see Decision 5.
- **Exit codes**: `std::process::exit` for 1/2; `Err` propagation for ≥3; see Decision 6.

## Phase 1: Design & Contracts

See [data-model.md](data-model.md) for internal types and [contracts/query-cli.md](contracts/query-cli.md)
for the full CLI contract.

### Key Design Decisions

**Preflight pipeline order** (all before opening Bolt connection):
1. Parse source(s) → `Vec<StatementEntry>`
2. Lint all entries via `cypher_data::lint::lint_source` → abort on any error
3. Classify each entry → abort if any Write and `--write` not set
4. Validate params: collect `$x` refs from all ASTs; cross-check against `ParamMap` → abort on missing required params
5. Connect and execute in order

**Parameter merging** (`--params` JSON file loaded first, then `--param` flags overwrite):
```rust
let mut params = load_params_file(args.params)?;  // may be empty
for s in &args.param {
    let (k, v) = parse_param_flag(s)?;
    params.insert(k, v);  // overwrite
}
```

**Lint reuse**: Call `cypher_data::lint::lint_source` directly — same function used by
`commands::lint`. No wrapper needed; we map `cypher_data::types::Diagnostic` to
`gram_diagnostics::Diagnostic` using `from_cypher_diagnostic` (already done in lint.rs —
extract to a shared helper in `commands/mod.rs` or duplicate locally for now).

**Write classification**: Implement a recursive tree-sitter node walker in `query.rs`:
```rust
fn classify(node: tree_sitter::Node, source: &[u8]) -> Classification { ... }
```
Walk depth-first; return `Write` on first write-clause node found.

**Result consumption**: Use `row.get::<serde_json::Value>(col)` for each column — this
avoids knowing the Cypher type ahead of time and lets `serde_json::Value` handle
serialization uniformly for both `--json` and table display.

**Table rendering**: Build column headers from `stream.columns()`, then collect all rows
into `Vec<Vec<Value>>` before rendering (required by `comfy-table`'s row-building API).

### `main.rs` changes

```rust
Commands::Query(args) => {
    if let Err(e) = commands::query::run(args, cli.neo4j).await {
        eprintln!("error: {e:#}");
        std::process::exit(2);
    }
    Ok(())
}
```

Exit code 1 is handled via `std::process::exit(1)` inside `commands::query::run`,
matching the lint pattern. Exit code 2 is for unexpected runtime errors that bubble
up as `Err`.

### `cli.rs` changes

Add `QueryArgs` struct and `Commands::Query(QueryArgs)` variant. See
[data-model.md](data-model.md) for the full struct definition.

Update `Cli.long_about` to include a `relate query` example.

## Implementation Sequence

Tasks are ordered to enable incremental testing at each step.

### Task 1: Scaffold (cli.rs + main.rs + empty query.rs)

Add `QueryArgs` to `cli.rs`, `Commands::Query` variant, update `main.rs` dispatch.
`commands::query::run` prints a stub message. Verify `relate query --help` renders.

**Test**: `assert_cmd` — `relate query --help` exits 0 and contains "query".

### Task 2: Source parsing (inline and file)

Implement `build_queue(args: &QueryArgs) -> Result<Vec<StatementEntry>>`:
- Enforce `query` and `expr` mutual exclusion → `process::exit(1)`
- `-e` flags → one `StatementEntry` per flag, source `StatementSource::Inline`
- File path → read, parse with tree-sitter-cypher, split statements, source `File { path, line }`

**Test**: Unit tests for `build_queue` with inline and single/multi-statement files.

### Task 3: Preflight Stage 1 — Lint

Implement `preflight_lint(queue: &[StatementEntry]) -> Result<()>`:
- Call `cypher_data::lint::lint_source` on each entry's text
- Render diagnostics via ariadne (reuse `from_cypher_diagnostic` from lint.rs)
- Call `process::exit(1)` on any Error-severity diagnostic

**Test**: Unit tests with valid and invalid Cypher strings.

### Task 4: Preflight Stage 2 — Write Classification

Implement `classify_statement(entry: &StatementEntry) -> Classification`:
- Re-parse with tree-sitter-cypher (or reuse the parse tree from Task 2 — thread it
  through `StatementEntry`)
- Walk AST depth-first looking for write clause node kinds

Implement `preflight_write(queue: &[StatementEntry], allow_write: bool)`:
- Classify all; if any Write and `!allow_write` → print diagnostic, `process::exit(1)`

**Test**: Unit tests for classification with CREATE, MERGE, SET, DELETE, MATCH-only.

### Task 5: Preflight Stage 3 — Parameter Validation

Implement `collect_param_refs(text: &str) -> HashSet<String>`:
- Walk AST for `parameter` node kind; extract the name (strip `$`)

Implement `preflight_params(queue: &[StatementEntry], params: &ParamMap)`:
- Union all refs across queue; check each required ref against `params`
- Missing → `process::exit(1)` with diagnostic

**Test**: Unit tests for ref collection and cross-check logic.

### Task 6: Parameter Parsing

Implement `parse_param_flag(s: &str) -> Result<(String, ParamValue)>`:
- Split on first `=`; type-coerce value per research Decision 1

Implement `load_params_file(path: Option<&PathBuf>) -> Result<ParamMap>`:
- Read and `serde_json::from_str` → map JSON object keys to `ParamValue::Json` entries

**Test**: Unit tests for coercion edge cases (numeric, booleans, `=` in value).

### Task 7: Execution and Result Collection

Implement `execute_queue(queue: &[StatementEntry], params: &ParamMap, neo4j: &Neo4jArgs) -> Result<Vec<QueryResult>>`:
- Build `Graph` connection
- For each entry: run `graph.execute(query(text).param(...))`, collect rows via `RowStream::next()`
- Collect `QuerySummary` from `stream.summary()` (neo4rs provides counters)
- Fail fast: first `Err` from neo4rs → `process::exit(2)` after printing error

**Test**: Integration test against testcontainers Neo4j (or skip with `#[ignore]`
if no container runtime available; mark as `integration`).

### Task 8: Output — Human-Readable Table

Implement `print_table(result: &QueryResult)`:
- Source header to stdout
- If no rows: print `(no rows returned)` or write summary
- Else: render with `comfy-table` UTF8_FULL preset
- Row count / summary line

**Test**: Snapshot tests for table output shape.

### Task 9: Output — JSON

Implement `print_json(results: &[QueryResult])`:
- Serialize to `Vec<JsonResult>` via serde; print to stdout

**Test**: Parse output with `serde_json` and assert shape.

### Task 10: Wire up `run()` + integration test

Implement `pub async fn run(args: QueryArgs, neo4j: Neo4jArgs) -> Result<()>` calling
all preflight functions then `execute_queue`, routing to table or JSON output.

Add end-to-end `assert_cmd` integration test: inline MATCH, inline CREATE without
`--write` (exit 1), inline CREATE with `--write` (exit 0 with summary).

---

## Complexity Tracking

No constitution violations. No complexity justification required.
