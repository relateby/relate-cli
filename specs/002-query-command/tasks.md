# Tasks: relate query — Single Query Execution

**Input**: Design documents from `specs/002-query-command/`
**Prerequisites**: plan.md ✅, spec.md ✅, research.md ✅, data-model.md ✅, contracts/query-cli.md ✅

**Organization**: Tasks are grouped by user story to enable independent implementation
and testing of each story.

## Format: `[ID] [P?] [Story] Description`

- **[P]**: Can run in parallel (different files or independent functions)
- **[Story]**: User story this task belongs to (US1–US4)

---

## Phase 1: Setup

**Purpose**: Add new dependency and create the module stub so the project compiles
throughout development.

- [x] T001 Add `comfy-table = "7"` to `[dependencies]` in `Cargo.toml`
- [x] T002 Add `pub mod query;` to `src/commands/mod.rs`
- [x] T003 Create `src/commands/query.rs` with a stub `pub async fn run(_args: QueryArgs, _neo4j: Neo4jArgs) -> anyhow::Result<()>` that prints "query: not yet implemented" and returns `Ok(())`

---

## Phase 2: Foundational (Blocking Prerequisites)

**Purpose**: Wire up the CLI plumbing and define the core types used by all user stories.
No user story can be tested until this phase is complete.

**⚠️ CRITICAL**: Complete before any Phase 3+ work.

- [x] T004 Add `QueryArgs` struct to `src/cli.rs` per `specs/002-query-command/data-model.md` (fields: `query: Option<PathBuf>`, `expr: Vec<String>`, `param: Vec<String>`, `params: Option<PathBuf>`, `write: bool`, `json: bool`); add `#[command(long_about = "...", after_help = "...")]` on `QueryArgs` with: (a) purpose sentence, (b) "Statements are linted before execution. Lint is syntactic — runtime errors (unknown labels, constraint violations) can still occur after lint passes." (c) "Write operations (CREATE, MERGE, SET, DELETE, REMOVE, FOREACH) require --write." (d) at least two usage examples per `contracts/query-cli.md`; also add `Query` entry to the root `Cli` `long_about`
- [x] T005 Add `Commands::Query(QueryArgs)` variant to the `Commands` enum in `src/cli.rs`; add the `/// Execute a Cypher query against Neo4j` doc comment
- [x] T006 Add `Commands::Query(args)` arm to the `match cli.command` block in `src/main.rs`; call `commands::query::run(args, cli.neo4j).await`; handle `Err` by printing to stderr and calling `std::process::exit(2)`, matching the pattern used for `Commands::Lint`
- [x] T007 [P] Define `StatementSource` enum in `src/commands/query.rs` (`Inline` and `File { path: PathBuf, line: u32 }` variants) with a `Display` impl: `Inline` → `<inline>`, `File` with line 1 on a single-statement file → `path/to/file.cypher`, `File` with line N > 1 → `path/to/file.cypher:N`
- [x] T008 [P] Define `StatementEntry` struct in `src/commands/query.rs` (`source: StatementSource`, `text: String`)
- [x] T009 [P] Define `ParamValue` enum (`Integer(i64)`, `Float(f64)`, `Boolean(bool)`, `String(String)`, `Json(serde_json::Value)`) and `type ParamMap = HashMap<String, ParamValue>` in `src/commands/query.rs`
- [x] T010 [P] Define `QueryResult` struct (`source: StatementSource`, `columns: Vec<String>`, `rows: Vec<Vec<serde_json::Value>>`, `summary: QuerySummary`) and `QuerySummary` struct (`nodes_created`, `nodes_deleted`, `relationships_created`, `relationships_deleted`, `properties_set`, `labels_added` — all `u64`) in `src/commands/query.rs`

**Checkpoint**: `cargo build` passes. `relate query --help` renders and shows all flags.

---

## Phase 3: User Story 1 — Inline Query Execution + Full Preflight Pipeline (Priority: P1) 🎯 MVP

**Goal**: A developer can run a Cypher statement inline (`-e`) against Neo4j and see
results in a formatted table. The complete preflight pipeline (lint → write classification
→ param validation) is built here so it is never partially implemented.

**Independent Test**: `relate query -e "MATCH (n) RETURN count(n) AS total"` prints a
table and exits 0. `relate query -e "CREATE (n:Test)"` exits 1 with a write-flag error
before connecting.

- [x] T011 [US1] Implement `fn build_queue_inline(exprs: &[String]) -> Vec<StatementEntry>` in `src/commands/query.rs`; each expr becomes one `StatementEntry` with `source: StatementSource::Inline`
- [x] T012 [P] [US1] Implement `fn preflight_lint(queue: &[StatementEntry])` in `src/commands/query.rs`; call `cypher_data::lint::lint_source` on each entry's text; convert diagnostics with `from_cypher_diagnostic` (copy or import from `commands::lint`); print errors via `ariadne` to stderr; call `std::process::exit(1)` if any error-severity diagnostic is found
- [x] T027 [P] [US1] Define `enum Classification { Read, Write { first_write_kind: String } }` in `src/commands/query.rs`
- [x] T028 [US1] Implement `fn classify_statement(text: &str) -> Classification` in `src/commands/query.rs`; parse with tree-sitter-cypher; walk the AST depth-first; return `Classification::Write { first_write_kind }` on the first node whose kind is one of: `"create_clause"`, `"merge_clause"`, `"set_clause"`, `"delete_clause"`, `"remove_clause"`, `"foreach_clause"`, `"call_clause"` (conservative — verify exact kind names against the grammar during implementation); return `Classification::Read` if none found
- [x] T029 [US1] Implement `fn preflight_write(queue: &[StatementEntry], allow_write: bool)` in `src/commands/query.rs`; classify each entry; if any is `Write` and `!allow_write`, print to stderr: `Error: write operation requires --write flag`, `  Statement: <first line of entry.text>`, `  Source: <entry.source>`, blank line, `  Re-run with --write to allow mutations.`; call `std::process::exit(1)`
- [x] T013 [P] [US1] Implement `async fn execute_queue(queue: &[StatementEntry], params: &ParamMap, neo4j: &Neo4jArgs) -> anyhow::Result<Vec<QueryResult>>` in `src/commands/query.rs`; build a `neo4rs::Graph` connection; for each entry call `graph.execute(neo4rs::query(&entry.text))` (no params yet); collect columns via `stream.columns()` and rows via `stream.next()` using `row.get::<serde_json::Value>(col)`; collect `QuerySummary` from `stream.summary()` counters; on any neo4rs error print to stderr and call `std::process::exit(2)`
- [x] T014 [P] [US1] Implement `fn print_table(result: &QueryResult)` in `src/commands/query.rs`; print `-- <source>` header; if `result.rows` is empty: check `result.summary` — if any counter is non-zero print an affected-nodes summary line (e.g. `Created 1 node, set 2 properties.` — enumerate only non-zero counters), otherwise print `(no rows returned)`; if rows are present build a `comfy_table::Table` with `UTF8_FULL` preset and `ContentArrangement::Dynamic`, set headers from `result.columns`, add rows from `result.rows` converting each `Value` to its string representation; print summary line `N row(s)`
- [x] T015 [US1] Update `pub async fn run(args: QueryArgs, neo4j: Neo4jArgs) -> anyhow::Result<()>` in `src/commands/query.rs` to: (1) enforce `args.query` and `args.expr` mutual exclusion — if both are set, print error to stderr and call `std::process::exit(1)`; (2) call `build_queue_inline` when `-e` flags are present; (3) call `preflight_lint`; (4) call `preflight_write(queue, args.write)` after `preflight_lint`; (5) call `execute_queue` with empty `ParamMap`; (6) call `print_table` for each result; (7) print final summary line `N statement(s) executed, M row(s) returned`

**Checkpoint**: `relate query -e "MATCH (n) RETURN count(n) AS total"` works end-to-end.
Lint errors exit 1 before connecting. `relate query -e "CREATE (n:Test)"` exits 1 with
a write-flag error. The complete preflight pipeline is operational.

---

## Phase 4: User Story 2 — File Execution (Priority: P1)

**Goal**: A developer can execute a `.cypher` file (single or multi-statement) and each
statement executes in order; the first failure aborts with an informative error.

**Independent Test**: Create `test.cypher` with `MATCH (n) RETURN count(n) AS total`,
run `relate query test.cypher`, observe results with filename as source header.

- [x] T016 [US2] Implement `fn build_queue_file(path: &std::path::PathBuf) -> anyhow::Result<Vec<StatementEntry>>` in `src/commands/query.rs`; read the file to a `String`; create a `tree_sitter::Parser` with `tree_sitter_cypher::language()`; parse the source; iterate child nodes of the root that have kind `"statement"` (verify node kind name against the grammar during implementation); extract each statement's text via its byte range; record the 0-based start line from `node.start_position().row` as the `line` field; return one `StatementEntry` per statement; if the resulting `Vec` is empty (file exists but contains no statements), return `Err(anyhow!("no statements found in {}", path.display()))`
- [x] T017 [US2] Update `build_queue` dispatch in `run()` to call `build_queue_file` when `args.query` is set; propagate the empty-file error from T016 by printing `Error: <message>` to stderr and calling `std::process::exit(1)`; if neither `args.query` nor `args.expr` is provided, print a usage error to stderr and call `std::process::exit(1)`
- [x] T018 [P] [US2] Add `assert_cmd` integration test in `tests/` (or `src/commands/query.rs` `#[cfg(test)]` block): `relate query <single-statement-file>` exits 0 and stdout contains the filename as source header
- [x] T019 [P] [US2] Add `assert_cmd` integration test: `relate query <multi-statement-file>` where second statement has a lint error exits 1 without connecting to Neo4j (verify no connection attempt by using an unreachable URI)

**Checkpoint**: Both inline and file-based queries work. Multi-statement files execute
in order. Lint preflight catches errors in any statement before connecting.

---

## Phase 5: User Story 3 — Named Parameters (Priority: P2)

**Goal**: A developer can supply `$x` parameter values via `--param NAME=VALUE` flags
or `--params FILE` (JSON), and missing required parameters are caught before connecting.

**Independent Test**: `relate query -e "MATCH (n:Person {name: $name}) RETURN n" --param name=Alice`
returns results for Alice; omitting `--param name` exits 1 with a clear error naming `$name`.

- [x] T020 [US3] Implement `fn parse_param_flag(s: &str) -> anyhow::Result<(String, ParamValue)>` in `src/commands/query.rs`; split on the first `=` only; coerce the value: fully-numeric integer → `ParamValue::Integer`, numeric with `.` → `ParamValue::Float`, `"true"`/`"false"` exact → `ParamValue::Boolean`, otherwise → `ParamValue::String`; return an error if no `=` is found
- [x] T021 [P] [US3] Implement `fn load_params_file(path: &std::path::PathBuf) -> anyhow::Result<ParamMap>` in `src/commands/query.rs`; read the file; `serde_json::from_str` into a `serde_json::Map<String, Value>`; convert each entry to `ParamValue::Json(value)`; return the resulting `ParamMap`
- [x] T022 [US3] Implement `fn build_param_map(args: &QueryArgs) -> anyhow::Result<ParamMap>` in `src/commands/query.rs`; start with `load_params_file` result (empty map if `args.params` is None); then insert each `parse_param_flag` result, overwriting on key collision (enforcing `--param` precedence over `--params`)
- [x] T023 [US3] Implement `fn collect_param_refs(text: &str) -> HashSet<String>` in `src/commands/query.rs`; parse with tree-sitter-cypher; walk the AST depth-first collecting node text for nodes with kind `"parameter"` (verify node kind name during implementation); strip the leading `$`; return the set of parameter names
- [x] T024 [US3] Implement `fn preflight_params(queue: &[StatementEntry], params: &ParamMap)` in `src/commands/query.rs`; union all `collect_param_refs` results across the queue; for each referenced name, if not present in `params` print an error to stderr (`Error: missing required parameter '$<name>'`, `Source: <source>`, `Hint: pass --param <name>=<value>`) and call `std::process::exit(1)`; emit a warning to stderr for any `params` key not referenced in any statement
- [x] T025 [US3] Update `execute_queue` in `src/commands/query.rs` to bind the `ParamMap` to each query; for each `(name, value)` pair in `params`, call `.param(name, ...)` on the neo4rs `query`; map `ParamValue` variants to the appropriate Rust types accepted by neo4rs (integer → `i64`, float → `f64`, boolean → `bool`, string → `&str`, json → `serde_json::Value`)
- [x] T026 [US3] Update `run()` to call `build_param_map`, then `preflight_params`, then pass the `ParamMap` to `execute_queue`

**Checkpoint**: `relate query -e "MATCH (n:Person {name: $name}) RETURN n" --param name=Alice`
returns correct results. Omitting `--param name` exits 1 before connecting.

---

## Phase 6: User Story 4 — Write Permission Validation (Priority: P2)

**Goal**: Explicitly verify write-protection behavior end-to-end. The `preflight_write`
implementation was delivered in Phase 3; this phase validates the user-facing behavior
with integration tests and ensures `--write` enables mutation queries.

**Independent Test**: `relate query -e "CREATE (n:Test)"` exits 1 with stderr containing
"write operation requires --write flag". `relate query -e "CREATE (n:Test)" --write` exits 0.

- [x] T036 [US4] Add `assert_cmd` integration test: `relate query -e "CREATE (n:Test)"` with an unreachable Neo4j URI exits 1 and stderr contains "write operation requires --write flag" (confirms no connection is attempted — the write check fires before Bolt)
- [ ] T037 [P] [US4] Add `assert_cmd` integration test: `relate query -e "MERGE (n:Test {id: 1}) RETURN n" --write` against a running Neo4j instance exits 0 and stdout contains a source header; mark `#[ignore]` if no live Neo4j available

**Checkpoint**: Write protection is verified end-to-end. All four user stories are
fully implemented and testable. The full preflight pipeline (lint → write → params)
runs before any Bolt connection opens.

---

## Phase 7: Polish & Cross-Cutting Concerns

**Purpose**: JSON output, help text quality, final wiring.

- [x] T031 [P] Implement `fn print_json(results: &[QueryResult])` in `src/commands/query.rs`; serialize to a `Vec<JsonResult>` (define a `#[derive(Serialize)] struct JsonResult` with `source: String`, `columns: Vec<String>`, `rows: Vec<serde_json::Value>` as objects, `summary: JsonSummary`); print via `serde_json::to_string_pretty`; write to stdout
- [x] T032 Update `run()` to route to `print_json` when `args.json` is set, otherwise `print_table` per result
- [x] T033 [P] Update the `long_about` on `Cli` in `src/cli.rs` to include a `relate query` usage example
- [x] T034 [P] Update the Architecture section in `CLAUDE.md` to add `query.rs    # async (neo4rs)` to the commands listing and note `comfy-table` in Key Dependencies
- [x] T035 [P] Add `assert_cmd` integration test: `relate query -e "MATCH (n) RETURN count(n)" --json` exits 0 and stdout parses as a valid JSON array with `source`, `columns`, `rows`, `summary` fields

---

## Dependencies & Execution Order

### Phase Dependencies

- **Setup (Phase 1)**: No dependencies — start immediately
- **Foundational (Phase 2)**: Requires Phase 1 — blocks all user stories
- **US1 (Phase 3)**: Requires Phase 2 — first MVP deliverable
- **US2 (Phase 4)**: Requires Phase 3 (reuses `preflight_lint`, `execute_queue`, `print_table`)
- **US3 (Phase 5)**: Requires Phase 4 (file parsing needed to test param refs in files)
- **US4 (Phase 6)**: Requires Phase 3 (write classification is implemented in Phase 3; Phase 6 adds integration tests that exercise the `--write` flag end-to-end)
- **Polish (Phase 7)**: Requires Phase 6 — all user stories complete

### Within Each Phase (parallel opportunities)

- **Phase 2**: T007, T008, T009, T010 are independent type definitions — [P]
- **Phase 3**: T012, T027, T013, T014 are independent functions — [P]; T028 depends on T027; T029 depends on T028; T015 depends on all preflight functions
- **Phase 5**: T020 and T021 are independent — [P]

### Parallel Execution Example: Phase 3 (US1)

```
# Independent functions — implement in parallel:
T012: preflight_lint()        — pure function, calls cypher-data
T027: Classification enum     — type definition only
T013: execute_queue()         — async, calls neo4rs
T014: print_table()           — pure function, calls comfy-table

# T028 depends on T027 (classify_statement uses Classification enum)
# T029 depends on T028 (preflight_write calls classify_statement)
# T015 (run() wiring) depends on T012, T029, T013, T014.
```

---

## Implementation Strategy

### MVP (User Story 1 only — Phase 1–3)

1. Complete Phase 1: Setup (T001–T003)
2. Complete Phase 2: Foundational (T004–T010) — verify `relate query --help` renders
3. Complete Phase 3: US1 (T011–T015)
4. **Validate**: `relate query -e "MATCH (n) RETURN count(n) AS total"` works end-to-end

### Incremental Delivery

- After Phase 3 (US1): inline queries work, full preflight pipeline operational ✅
- After Phase 4 (US2): file-based queries work ✅
- After Phase 5 (US3): parameterized queries work ✅
- After Phase 6 (US4): write protection validated end-to-end ✅
- After Phase 7 (Polish): `--json` output and full documentation ✅

---

## Notes

- [P] tasks operate on different functions/files with no shared state — safe to parallelize
- Verify tree-sitter node kind names (`"statement"`, `"parameter"`, `"create_clause"`, etc.) against the actual grammar during T016 and T028 — they are the expected names but must be confirmed by inspecting a parse tree at runtime
- `from_cypher_diagnostic` in `commands/lint.rs` should be extracted to a shared helper in `commands/mod.rs` before T012 to avoid duplication
- Integration tests that require a live Neo4j connection should be marked `#[ignore]` by default and documented in a `tests/README.md` with setup instructions
- Commit after each phase checkpoint
