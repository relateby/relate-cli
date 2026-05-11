# Tasks: Batch Execution (Milestone 3)

**Input**: Design documents from `specs/004-batch-execution/`
**Prerequisites**: plan.md ✅, spec.md ✅, research.md ✅, data-model.md ✅, contracts/query-cli.md ✅

**Organization**: Tasks are grouped by user story. US1 (CSV) and US2 (JSON/JSONL)
share the `RowReader` trait and the apply-mode dispatcher, so the trait and
dispatcher are built in the Foundational phase and the format-specific readers
land in each story. US3 (transaction modes) builds on the apply dispatcher and
is implemented alongside US1 because the dispatcher needs `BatchMode` from day
one. US4 (progress) and US5 (per-row JSON) are additive renderers on top of
US1–US3 and are independently testable.

## Format: `[ID] [P?] [Story] Description`

- **[P]**: Can run in parallel with other [P] tasks in the same phase (different files or non-conflicting sections of the same file)
- **[Story]**: Which user story this task belongs to
- All tasks include exact file paths

---

## Phase 1: Setup

**Purpose**: Add the one new crate dependency before any implementation begins.

- [x] T001 Add `csv = "1.3"` to `[dependencies]` in `Cargo.toml`; run `cargo build` to verify the crate resolves and compiles

**Checkpoint**: `cargo build` succeeds with the new dependency on the project's MSRV.

---

## Phase 2: Foundational (Blocking Prerequisites)

**Purpose**: Shared infrastructure consumed by every batch-execution user story.
All changes are additive — existing M1/M2 invocations remain unchanged.

**⚠️ CRITICAL**: No user story work can begin until this phase is complete.

- [x] T002 Add three new fields to `QueryArgs` in `src/cli.rs`: `apply: Option<PathBuf>` (`--apply`), `batch: Option<usize>` (`--batch`), `atomic: bool` (`--atomic`); update `long_about` and `after_help` to mention the new flags and add `--apply` examples
- [x] T003 [P] Add `BatchMode` enum (`Batched(usize)` and `Atomic`) plus `BatchMode::from_args(args: &QueryArgs) -> Result<BatchMode>` in `src/commands/query.rs`; enforce the three mutex rules — `--batch` xor `--atomic`, `--batch 0` rejected, `--batch`/`--atomic` without `--apply` rejected — and the `1000` default when neither is set
- [x] T004 [P] Define the `RowReader` trait (`next_row(&mut self) -> Option<Result<ParamMap>>` and `total_hint(&self) -> Option<usize>`) in `src/commands/query.rs`; this trait is the abstraction US1 and US2 will plug their format-specific readers into
- [x] T005 Add `PeekableRowReader` struct in `src/commands/query.rs` wrapping `Box<dyn RowReader>` with `open(path: &Path)`, `first_row(&self) -> Option<&ParamMap>`, `next_row(&mut self) -> Option<Result<ParamMap>>`, and `total_hint(&self) -> Option<usize>`; `open` reads row 0 eagerly into the `first` cache; `next_row` yields the cached row first then delegates
- [x] T006 Add the `open_row_reader(path: &Path) -> Result<Box<dyn RowReader>>` factory in `src/commands/query.rs` that dispatches on file extension (`.csv`, `.json`, `.jsonl`); for now, each branch returns `bail!("unimplemented")` placeholders to be filled by US1/US2 — but the extension validator and the "must be .csv/.json/.jsonl" error message MUST be complete here so the contract diagnostic is in place
- [x] T007 [P] Add `row: Option<usize>` field to `QueryResult` struct in `src/commands/query.rs`; update every existing construction site (Milestone 1/2 paths) to `row: None`; this keeps M1/M2 JSON output bit-for-bit identical
- [x] T008 [P] Add `row: Option<usize>` field to `JsonResult` struct in `src/commands/query.rs` with `#[serde(skip_serializing_if = "Option::is_none")]`; update the existing `print_json` mapping to pass `row: r.row` from `QueryResult` through to `JsonResult`
- [x] T009 Add the `execute_queue_apply` async function signature in `src/commands/query.rs` — `async fn execute_queue_apply(queue: &[StatementEntry], constants: &ParamMap, reader: PeekableRowReader, mode: BatchMode, neo4j: &Neo4jArgs, json: bool) -> Result<Vec<QueryResult>>` — with a stub body that returns `bail!("unimplemented")`. The body is filled in by US3 (transaction modes); US1/US2 only depend on the function existing and on the readers it consumes
- [x] T010 Wire mutex checks into `run()` in `src/commands/query.rs`: (a) `--apply` and `[PARAMS]` are mutually exclusive; (b) call `BatchMode::from_args(&args)?` early when `--apply` is set; (c) when `--apply` is set, take the `--apply` path instead of the existing `execute_queue` path (initially just call the stub `execute_queue_apply`)
- [x] T011 Add unit tests in the `mod tests` block of `src/commands/query.rs` covering `BatchMode::from_args` — defaults to `Batched(1000)`, accepts `--batch 500`, rejects `--batch 0`, rejects `--batch` + `--atomic`, rejects flags without `--apply`

**Checkpoint**: `cargo build` and `cargo test` pass; M1/M2 behavior unchanged; the apply path is reachable but every format returns "unimplemented".

---

## Phase 3: User Story 1 — Apply a Query Across CSV Rows (Priority: P1) 🎯 MVP slice 1

**Goal**: `relate query create_person --apply people.csv --write` runs the
query once per CSV row, mapping headers to parameter names with the same
type coercion as the `--param` flag.

**Independent Test**: Create a small CSV file with a header and 2–3 rows
plus a matching `.cypher` file in `./cypher/`. Run with `--apply file.csv
--write`; verify every row is applied. Confirm the first row's columns
satisfy preflight before any connection opens (test by giving a CSV that
*doesn't* have the required column and asserting exit code 1 with no
connection attempted — use a bogus `--uri` to prove the connection isn't
opened).

- [x] T012 [P] [US1] Implement `CsvRowReader` in `src/commands/query.rs` wrapping `csv::Reader<File>`; constructor reads the header row, validates that headers are non-empty and unique (return descriptive error otherwise), stores headers, and exposes `next_row` returning a `ParamMap` whose keys are header names and whose values follow `--param` coercion rules (integer / float / boolean / string; empty cell → `String("")`)
- [x] T013 [US1] Wire `CsvRowReader` into `open_row_reader` for the `.csv` branch
- [x] T014 [P] [US1] Extract the `--param`/`--params` coercion logic from `build_param_map` in `src/commands/query.rs` into a reusable `coerce_param_value(s: &str) -> ParamValue` helper if not already factored; `CsvRowReader::next_row` MUST call this helper so coercion behavior is identical to `--param` flags
- [x] T015 [US1] Add an integration test `query_apply_csv_basic` in `tests/cli_integration.rs` (gated by `NEO4J_PASSWORD` like existing query tests) using `tempfile` for a CSV with 3 rows and a matching `cypher/create_person.cypher`; assert exit code 0 and verify all 3 nodes exist
- [x] T016 [P] [US1] Add an integration test `query_apply_csv_missing_column_preflight_fails` in `tests/cli_integration.rs` that runs against a deliberately wrong `--uri` (e.g. `bolt://127.0.0.1:1`) and asserts exit code 1 with stderr containing "missing required parameter" — proving preflight fails before the connection is attempted
- [x] T017 [P] [US1] Add an integration test `query_apply_with_param_constants` in `tests/cli_integration.rs` verifying that `--param tenant=acme` is merged into every row's parameter set and that `--param` wins on key conflict with a CSV column of the same name
- [x] T018 [P] [US1] Add a unit test in `mod tests` of `src/commands/query.rs` for CSV header validation: empty header and duplicate header names each return distinct error messages

**Checkpoint**: CSV `--apply` works end-to-end with the default `Batched(1000)` mode. US1 is independently testable. (The transaction-mode internals are stubbed via US3; this story uses only the default and asserts row counts at the database, not commit boundaries.)

---

## Phase 4: User Story 2 — Apply a Query Across JSON or JSONL Records (Priority: P1)

**Goal**: `--apply` accepts `.json` (top-level array of objects) and `.jsonl`
(one object per line), preserving JSON value types (no string-coercion).

**Independent Test**: Provide a `.json` and a `.jsonl` file with the same
records as US1's CSV test and verify the resulting Neo4j state is identical,
plus that JSON integer values become Cypher integers (not strings).

- [x] T019 [P] [US2] Implement `JsonArrayRowReader` in `src/commands/query.rs` wrapping `std::vec::IntoIter<serde_json::Value>`; constructor reads the full file, parses with `serde_json::from_reader`, validates the top-level is an array, validates each element is an object, returns `total_hint = Some(len)`; `next_row` consumes one object and returns a `ParamMap` whose values preserve JSON types (integer → `ParamValue::Integer`, decimal → `Float`, bool → `Boolean`, string → `String`, null → `Json(Null)`, array/object → `Json(...)`)
- [x] T020 [P] [US2] Implement `JsonlRowReader` in `src/commands/query.rs` wrapping `std::io::Lines<BufReader<File>>`; tracks current line number; `next_row` skips empty lines, parses non-empty lines with `serde_json::from_str`, requires top-level object on each line, surfaces malformed lines as `Err` with `"invalid JSON on line N: <error>"`; `total_hint` returns `None` (streaming)
- [x] T021 [US2] Wire `JsonArrayRowReader` and `JsonlRowReader` into `open_row_reader` for the `.json` and `.jsonl` branches respectively; remove the corresponding `bail!("unimplemented")` stubs
- [x] T022 [P] [US2] Add a JSON-value-to-ParamValue helper in `src/commands/query.rs` (e.g. `param_value_from_json(v: serde_json::Value) -> ParamValue`) used by both JSON readers; this isolates type-preservation in one place
- [x] T023 [P] [US2] Add unit tests in `mod tests` of `src/commands/query.rs`: (a) JSON array with non-object element rejected; (b) JSON non-array top-level rejected with the contract error message; (c) JSONL malformed line surfaces the line number; (d) `param_value_from_json` preserves integer/float/bool/string/null
- [x] T024 [US2] Add an integration test `query_apply_json_array` in `tests/cli_integration.rs` using a small `.json` array
- [x] T025 [P] [US2] Add an integration test `query_apply_jsonl` in `tests/cli_integration.rs` using a small `.jsonl` file with both well-formed and one malformed line in a separate negative test
- [x] T026 [P] [US2] Add an integration test `query_apply_unknown_extension` in `tests/cli_integration.rs` verifying a `.txt` file is rejected with exit 1 and the contract error message ("--apply requires a .csv, .json, or .jsonl file")

**Checkpoint**: All three file formats work for `--apply`. US2 is independently testable. JSON value types are preserved through to Cypher.

---

## Phase 5: User Story 3 — Control Transaction Batching (Priority: P1)

**Goal**: Default `--batch 1000`, configurable `--batch N`, and `--atomic`
each behave per the three-tier transaction model with correct commit /
rollback boundaries and clear error messages on partial failure.

**Independent Test**: Use a 5-row data file with a uniqueness constraint
pre-configured so row 3 fails. Expected outcomes:

- `--batch 2`: rows 1–2 commit (batch 1), row 3 fails inside batch 2 →
  2 rows committed, 1 rolled back, rows 4–5 never attempted.
- `--atomic`: row 3 fails inside the single transaction →
  0 rows committed, the run aborts after row 3.
- Default `--batch 1000`: all 5 rows fall into batch 1; row 3 fails →
  0 rows committed, 3 rows rolled back.

Verify each case end-to-end against a real Neo4j; assertions are on
post-run row visibility (consistent with SC-003).

- [x] T027 [US3] Replace the stub body of `execute_queue_apply` in `src/commands/query.rs` with the real implementation: open Bolt connection once via `Graph::new`; loop using `PeekableRowReader::next_row`; merge each row's `ParamMap` with `constants` (constants win on conflict — same precedence as `--param`); for multi-statement queues, run every statement per row; on error, propagate `BatchError` with row index, statement source, underlying error
- [x] T028 [US3] Inside `execute_queue_apply`, implement the `BatchMode::Batched(n)` path: open `Txn` lazily; track `rows_in_batch`; on `rows_in_batch == n` call `tx.commit().await` and start a new `Txn`; on row-level error, `tx.rollback().await` and return a `BatchError` reporting `rows_committed` (sum of all prior fully-committed batches) and `rows_rolled_back` (rows in the current batch including the failing row); after the loop, commit the final partial batch
- [x] T029 [US3] Inside `execute_queue_apply`, implement the `BatchMode::Atomic` path: open one `Txn` before the loop, run every row inside it, commit once after the loop; on any row-level error, `tx.rollback().await` and return a `BatchError` with `rows_committed = 0`, `rows_rolled_back = <rows seen including failing>`
- [x] T030 [P] [US3] Add a `BatchError`-to-stderr renderer in `src/commands/query.rs` matching the exact format in `contracts/query-cli.md`: "Error on row N: <code>" / "  Source: <source>" / "  N rows committed (K batches), M rows in current batch rolled back." (or "Transaction rolled back. 0 rows committed." for atomic) / "  Underlying: ..."; exit code 2
- [x] T031 [US3] Wire empty-input handling in `run()` (`src/commands/query.rs`): if `PeekableRowReader::first_row()` is `None`, run preflight against an empty `ParamMap`; if preflight passes (no required params), exit 0 silently; if preflight fails (required params), surface the error with hint "no input rows found in <file>"
- [x] T032 [P] [US3] Add a unit test in `mod tests` of `src/commands/query.rs` verifying the `BatchError` renderer output exactly matches the contract format for both `Batched(N)` and `Atomic` modes
- [x] T033 [US3] Add integration tests `query_apply_batch_default`, `query_apply_batch_1`, `query_apply_atomic_rollback_on_failure`, and `query_apply_batched_partial_commit_on_failure` in `tests/cli_integration.rs` covering the three transaction modes and their respective commit/rollback semantics. Use a `Person(name)` uniqueness constraint to force a deterministic failure on a known row
- [x] T034 [P] [US3] Add an integration test `query_apply_empty_csv_with_no_required_params` verifying exit 0 and no output for an empty CSV when no parameters are required, and `query_apply_empty_csv_with_required_params` asserting exit 1 with the "no input rows" hint

**Checkpoint**: All three transaction modes work correctly. Partial-commit accounting is exact. US3 is independently testable using only one constraint-violation fixture.

---

## Phase 6: User Story 4 — Monitor Progress During Long Runs (Priority: P2)

**Goal**: Per-row or per-batch progress lines stream to stderr during
`--apply` runs longer than one row, leaving stdout clean for `--json`.

**Independent Test**: Run `--apply` with a small file and capture stderr
into a file (`2> progress.log`); verify at least one progress line is
written and that stdout (especially under `--json`) contains only the
result payload.

- [x] T035 [US4] Inside `execute_queue_apply` in `src/commands/query.rs`, emit progress lines to stderr per the contract: `[N/M] applied row N` when `total_hint` is `Some(M)`, `[N/?] applied row N` when `None`; for `Batched(N)`, append `(batch K: rows_in_batch/N)` and `(batch K committed)` after commit; for `Atomic`, append `(atomic — 1 transaction)`. The frequency is per-row for runs ≤ 100 rows; for larger runs, throttle to every Nth row where N = max(1, total / 100) or 100 rows when the total is unknown
- [x] T036 [US4] Emit the final summary line to stdout (not stderr): `N rows applied across K batches.` (Batched) or `N rows applied in 1 atomic transaction.` (Atomic); under `--json`, the final summary is replaced by the JSON array output and no additional stdout text is written
- [x] T037 [P] [US4] Add an integration test `query_apply_progress_to_stderr` in `tests/cli_integration.rs` capturing stderr separately and asserting it contains at least one `[N/...]` line, while stdout under `--json` is valid JSON with no progress noise
- [x] T038 [P] [US4] Add an integration test `query_apply_summary_to_stdout` verifying the final summary line appears on stdout (default, non-JSON mode)

**Checkpoint**: Progress is visible to the user, never pollutes stdout, and `--json` consumers can parse stdout without preprocessing.

---

## Phase 7: User Story 5 — Machine-Readable Per-Row Results (Priority: P2)

**Goal**: Under `--json` with `--apply`, stdout is a JSON array of per-row
result objects, each carrying a `row` index. The schema is identical to
Milestone 1 plus the `row` field.

**Independent Test**: Run `relate query <name> --apply file.json --write
--json` and pipe stdout into a JSON parser (e.g., `jq length`); the output
parses, and each element has the expected `row` index, source, and
summary fields.

- [x] T039 [US5] Inside `execute_queue_apply` in `src/commands/query.rs`, set `QueryResult.row = Some(row_index)` for every result produced by an apply iteration; the existing `print_json` (which already passes `row` through to `JsonResult` from T008) will then emit the `row` field automatically
- [x] T040 [P] [US5] Add an integration test `query_apply_json_output_schema` in `tests/cli_integration.rs` running `--apply file.jsonl --write --json` against a 3-row file and asserting (via `serde_json::from_str` of captured stdout): (a) top-level is array of length 3; (b) each element has `row` field with values 0, 1, 2 in order; (c) each element preserves the M1 schema (`source`, `columns`, `rows`, `summary`, `is_write`)
- [x] T041 [P] [US5] Add a regression integration test `query_single_row_json_omits_row_field` confirming that an M1-style invocation (no `--apply`) produces JSON without the `row` field — verifying the `skip_serializing_if` is wired correctly and we haven't broken the M1/M2 schema
- [x] T042 [P] [US5] Add an integration test `query_apply_json_terminates_at_failing_row` confirming that under `--json` with a mid-batch failure, the stdout JSON array contains results for rows up to (but not including) the failing row, and the error renders on stderr

**Checkpoint**: Per-row JSON output is consumable by standard JSON tools. Milestone 1/2 JSON output is bit-for-bit unchanged.

---

## Phase 8: Polish & Cross-Cutting Concerns

**Purpose**: Documentation, agent surfaces, and final validation.

- [x] T043 [P] Update `src/cli.rs` `QueryArgs` `long_about` and `after_help` strings to include the final wording of `--apply`, `--batch`, `--atomic`, and at least one example for each — matching the wording in `contracts/query-cli.md` (sections "Options" and "Usage Examples")
- [x] T044 [P] Update `skills/relate-query/SKILL.md` to add a "Batch execution" workflow section: numbered steps for format detection, transaction-mode choice, preflight, progress monitoring, and post-run validation; add an anti-rationalization row "silently picking --batch 1000 for a large dataset" → "confirm the transaction mode with the user when the input file has >10,000 rows or unknown size"
- [x] T045 [P] Update `skills/relate/SKILL.md` routing table to mention that the `relate-query` skill now covers batch execution (`--apply`)
- [x] T046 Update `CLAUDE.md` "Architecture" section if the `src/commands/query.rs` file has grown past its M2 size — note the (still-single-file) layout and the new internal modules: `RowReader` trait + impls, `BatchMode`, `execute_queue_apply`
- [x] T047 Run `cargo fmt --check && cargo clippy --all-targets -- -D warnings && cargo test` and resolve any issues; verify M1/M2 integration tests still pass unchanged
- [x] T048 [P] Walk through every step of `specs/004-batch-execution/quickstart.md` against a running Neo4j; fix any drift between the quickstart prose and actual command output before the milestone is considered complete

**Checkpoint**: Tests green, lints clean, docs updated, skills updated. Milestone 3 is complete.

---

## Dependencies & Execution Order

### Phase Dependencies

- **Setup (Phase 1)**: No dependencies — can start immediately.
- **Foundational (Phase 2)**: Depends on Phase 1; BLOCKS all user stories.
- **US1 (Phase 3) — CSV apply**: Depends on Phase 2.
- **US2 (Phase 4) — JSON / JSONL apply**: Depends on Phase 2; independent of US1.
- **US3 (Phase 5) — transaction modes**: Depends on Phase 2; replaces the stub body of `execute_queue_apply`. US3 is *technically* independent of US1 and US2 but in practice US1 or US2 must land first so the apply path has at least one working format to test against. Recommended order: US1 → US3 → US2 (US1 unlocks end-to-end testing; US3 makes the mode coverage real; US2 adds the remaining formats).
- **US4 (Phase 6) — progress**: Depends on US3 (progress emitters live inside `execute_queue_apply`).
- **US5 (Phase 7) — per-row JSON**: Depends on US3 (sets `QueryResult.row`); foundational T007/T008 already wired the `row` field through to JSON, so US5 is largely a verification phase.
- **Polish (Phase 8)**: Depends on all user stories that are in scope for the release.

### Within Each User Story

- Tests are integration-heavy; they don't need to be written before implementation, but the unit tests in T011, T018, T023, T032 should land alongside or just after the code they cover.
- Within US1: implement `CsvRowReader` (T012) before wiring it in (T013) before testing it (T015–T017).
- Within US2: the two readers (T019, T020) can be developed in parallel; both feed into T021 (wiring) before tests land.
- Within US3: T027 → T028 → T029 are sequential (each adds to `execute_queue_apply`); T030 (renderer) is parallelizable with T027/T028.

### Parallel Opportunities

- Phase 2: T003, T004, T007, T008 are all `[P]` — they touch different code regions.
- Phase 3: T012 and T014 are `[P]` (different functions); the three integration tests T015–T017 are `[P]` (different test fns); the unit test T018 is `[P]`.
- Phase 4: T019, T020, T022, T023 are all `[P]` — different functions/tests.
- Phase 5: T030 and T032 are `[P]` with T027/T028 (renderer can be drafted while transaction logic lands); T033–T034 are `[P]` integration tests.
- Phase 6: T037 and T038 are `[P]`.
- Phase 7: T040–T042 are all `[P]`.
- Phase 8: T043–T045 and T048 are all `[P]`.

---

## Parallel Example: User Story 1

```bash
# Inside US1, after T012 lands, run these together:
Task: "T014 [P] [US1] Extract coerce_param_value helper in src/commands/query.rs"
Task: "T018 [P] [US1] Add CSV header validation unit tests in mod tests of src/commands/query.rs"

# Then T015, T016, T017 (integration tests) can all be written in parallel:
Task: "T015 [US1] query_apply_csv_basic in tests/cli_integration.rs"
Task: "T016 [P] [US1] query_apply_csv_missing_column_preflight_fails in tests/cli_integration.rs"
Task: "T017 [P] [US1] query_apply_with_param_constants in tests/cli_integration.rs"
```

---

## Implementation Strategy

### MVP slice (Stories US1 + US3, default mode only)

1. Complete Phase 1 (Setup) and Phase 2 (Foundational).
2. Implement US1 (Phase 3) — CSV applies end-to-end with the stubbed apply dispatcher returning a hard error for any actual Neo4j call.
3. Implement US3 (Phase 5) up through T028 — default `Batched(1000)` works.
4. **STOP and VALIDATE**: `relate query <name> --apply file.csv --write` actually loads data. This is the smallest demoable slice.
5. Continue with the rest of US3 (`--batch N`, `--atomic`, partial-commit reporting) and then US2 (JSON / JSONL).

### Incremental Delivery

1. Setup + Foundational → unblocks all stories.
2. US1 (CSV) + minimal US3 (default batch) → MVP demo.
3. US3 full → all transaction modes covered.
4. US2 (JSON / JSONL) → all input formats covered.
5. US4 (progress) + US5 (per-row JSON) → release-ready ergonomics.
6. Polish → ship.

### Parallel Team Strategy

After Phase 2:
- Developer A: US1 (CSV reader + tests).
- Developer B: US2 (JSON/JSONL readers + tests).
- Developer C: US3 (transaction modes + renderer).

US4 and US5 are best done by whoever lands US3, because they sit inside
the same `execute_queue_apply` function.

---

## Notes

- `[P]` tasks = different files or non-conflicting regions of the same file with no dependencies on incomplete tasks in the same phase.
- All apply-related code lives in `src/commands/query.rs`; if that file becomes hard to navigate, splitting into `src/commands/query/` submodule is an out-of-scope follow-up RFC.
- Integration tests that require Neo4j must remain gated by `NEO4J_PASSWORD` like the existing M1/M2 query tests; they should not fail CI when the env var is absent.
- Per Constitution III, the milestone is not complete until `skills/relate-query/SKILL.md` reflects the new capabilities (T044).
- Verify M1/M2 JSON output is bit-for-bit unchanged at every step — the `skip_serializing_if` on `row` is the load-bearing invariant for backward compatibility.
