# Feature Specification: Batch Execution

**Feature Branch**: `004-batch-execution`  
**Created**: 2026-05-11  
**Status**: Draft  
**Input**: User description: "Milestone 3 of RFC-003 — Batch Execution: apply a single parameterized query once per row of a data file (CSV, JSON array, or JSONL), with configurable transaction batching and progress reporting."

## User Scenarios & Testing *(mandatory)*

### User Story 1 - Apply a Query Across CSV Rows (Priority: P1)

A developer has a CSV file of records (e.g., `people.csv` with columns `name,age,home`) and a parameterized query (e.g., `create_person`). They want to insert every row by running a single command, the same way they would use `LOAD CSV` but without needing the file to be reachable by the Neo4j server.

**Why this priority**: This is the primary use case for batch execution and the most common starting point for users coming from `LOAD CSV`. CSV is the dominant tabular interchange format for ad-hoc data loads.

**Independent Test**: Can be fully tested by placing a small CSV file next to a parameterized `.cypher` file and running `relate query <name> --apply people.csv --write`, then verifying every row was applied to Neo4j.

**Acceptance Scenarios**:

1. **Given** a CSV file with a header row and N data rows, **When** the user runs `relate query create_person --apply people.csv --write`, **Then** the query is executed once per data row with each row's columns mapped to parameters by header name.
2. **Given** a CSV row is missing a value for a required parameter, **When** preflight runs against the first row, **Then** the run aborts before any Neo4j connection is opened with an error identifying the missing parameter.
3. **Given** `--param environment=prod` is supplied alongside `--apply`, **When** the query runs, **Then** the constant `environment` value is injected into every row's parameter set in addition to the row-derived parameters.
4. **Given** `--apply people.csv` and a positional `[PARAMS]` map are both provided, **When** the user runs the command, **Then** an error reports that these two input modes are mutually exclusive.

---

### User Story 2 - Apply a Query Across JSON or JSONL Records (Priority: P1)

A developer's source data is structured as a JSON array (`people.json` containing `[{...}, {...}]`) or as newline-delimited JSON (`people.jsonl`, one object per line). They want to apply a parameterized query to each object without converting their data to CSV first.

**Why this priority**: JSON/JSONL are increasingly common for structured data exports and avoid the type-coercion ambiguity of CSV (everything-is-a-string). JSONL specifically enables streaming over large datasets without loading the full file into memory.

**Independent Test**: Can be fully tested by providing a `.json` or `.jsonl` file to `--apply` and verifying each object's keys map directly to query parameters without type coercion artifacts.

**Acceptance Scenarios**:

1. **Given** a JSON array file `[{ "name": "Alice", "age": 30 }, { "name": "Bob", "age": 25 }]`, **When** the user runs `--apply people.json`, **Then** each object's keys map to parameters and types are preserved (integer stays integer, etc.).
2. **Given** a JSONL file with one object per line, **When** the user runs `--apply people.jsonl`, **Then** each line is parsed independently and applied as a separate query execution.
3. **Given** a malformed JSON line in a JSONL file, **When** that line is reached, **Then** the run aborts with an error identifying the offending line number.
4. **Given** a JSON file whose top-level value is not an array, **When** the user runs `--apply`, **Then** an error reports that JSON input must be an array of objects.

---

### User Story 3 - Control Transaction Batching (Priority: P1)

A developer running a large batch wants to control how many rows are committed per transaction—either to optimize throughput (commit every 1000 rows), to maximize durability (commit every row), or to guarantee atomicity (single transaction, full rollback on any error).

**Why this priority**: Transaction semantics directly affect correctness and operational behavior. Without configurable batching, users either lose all in-flight work on a single bad row (full atomicity by default) or pay per-row commit overhead. Both extremes are common requirements.

**Independent Test**: Can be fully tested by running a controlled batch with a deliberate error mid-stream and verifying which rows were committed under each transaction mode (`--batch 1`, `--batch 100`, `--atomic`).

**Acceptance Scenarios**:

1. **Given** no `--batch` or `--atomic` flag is set, **When** the user runs `--apply` over N rows, **Then** the tool commits every 1000 rows by default.
2. **Given** `--batch 100`, **When** an error occurs on row 150, **Then** rows 1–100 are committed, rows 101–150 are rolled back, and the error message reports both counts.
3. **Given** `--atomic`, **When** an error occurs on any row, **Then** zero rows are committed and the error message reports that the entire transaction was rolled back.
4. **Given** both `--batch 500` and `--atomic` are provided, **When** the user runs the command, **Then** an error reports that these flags are mutually exclusive.
5. **Given** `--batch 1`, **When** the user runs `--apply` over N rows, **Then** each row is committed in its own transaction.

---

### User Story 4 - Monitor Progress During Long Runs (Priority: P2)

A developer running `--apply` over thousands of rows wants visible feedback that the operation is progressing—how many rows have been processed, and where execution stopped if a failure occurs.

**Why this priority**: Without progress feedback, users running multi-minute batches cannot distinguish a slow run from a hung one. Progress output is a quality-of-life requirement, not a correctness requirement, so it is P2.

**Independent Test**: Can be fully tested by running `--apply` against a JSONL file of more than one row and verifying that progress lines appear on stderr as rows are processed.

**Acceptance Scenarios**:

1. **Given** an `--apply` run over more than one row, **When** the run is in progress, **Then** per-row or per-batch progress lines are printed to stderr (not stdout).
2. **Given** `--json` output mode and an `--apply` run, **When** progress is printed to stderr, **Then** the JSON array on stdout remains parseable and unpolluted by progress text.
3. **Given** an error occurs at row M of N, **When** the run aborts, **Then** the final stderr line identifies the failing row and the underlying Neo4j error.

---

### User Story 5 - Machine-Readable Per-Row Results (Priority: P2)

A developer wants to capture the result of every row in a batch—including the row index and Neo4j's per-statement summary (nodes created, properties set, etc.)—for post-run analysis or auditing.

**Why this priority**: Users scripting batch loads often need to confirm which rows succeeded and what each row's effect was. Programmatic consumers need a stable JSON schema that extends the Milestone 1 result format with a row index.

**Independent Test**: Can be fully tested by running `--apply --json` over a small data file and parsing the resulting JSON array, verifying each element corresponds to one input row in order.

**Acceptance Scenarios**:

1. **Given** `--json` is set with `--apply`, **When** the run completes, **Then** stdout contains a JSON array with one result object per applied row, in input order, each carrying a `"row"` index field.
2. **Given** the run aborts on row M, **When** `--json` is set, **Then** the stdout array contains results for rows successfully executed up to (and not including) the failing row; the abort error is reported on stderr.
3. **Given** the per-row result format with `--apply`, **When** compared to Milestone 1's single-statement JSON shape, **Then** the only schema difference is the added `"row"` field on each element.

---

### Edge Cases

- What happens when the data file is empty (zero data rows after the CSV header, or an empty JSON array)? See FR-017 — answer depends on whether the query has required parameters.
- How does the system handle a CSV file with quoted fields containing commas, embedded newlines, or escaped quotes?
- How does the system distinguish between a CSV cell containing the literal string `"null"` and an intended Cypher NULL? (Out of scope: CSV cannot natively express NULL; users wanting NULL must use JSON/JSONL or restructure the query.)
- What happens when the data file contains far more columns/keys than the query references—are unused parameters silently ignored, or does Stage 3 warn?
- How does the system behave when the data file is larger than available memory? (Specifically: JSON array requires full-file parse; JSONL streams.)
- What happens when `--apply` is used with a query source containing multiple statements? Each row runs every statement in the queue in order, within the active transaction. Errors identify both the failing row index and the statement source label (file:line or file/stmt). Under `--atomic`, all statements across all rows share one transaction.
- What happens when a row's parameter type does not match a cypherdoc `@param` type declaration (e.g., a string where an integer is expected)?
- How does `--atomic` interact with a query that performs schema operations (constraint creation, index creation) that cannot be wrapped in user transactions?

## Requirements *(mandatory)*

### Functional Requirements

- **FR-001**: The tool MUST accept `--apply <FILE>` to indicate that the query should be executed once per row of the file.
- **FR-002**: The tool MUST support three data file formats for `--apply`: CSV (with header row), JSON array of objects, and JSONL (one JSON object per line). Format MUST be detected by file extension (`.csv`, `.json`, `.jsonl`).
- **FR-003**: For each row, the tool MUST construct a parameter set by mapping CSV header names or JSON object keys to query parameter names of the same name.
- **FR-004**: `--apply` MUST be mutually exclusive with the positional `[PARAMS]` map literal; providing both is an error caught before execution begins.
- **FR-005**: When both `--apply` and `--param` flags are provided, the `--param` values MUST be merged into every row's parameter set as constants. `--param` values take precedence on key conflicts with row-derived values.
- **FR-006**: The preflight pipeline MUST run against a synthetic parameter set derived from the first data row before any Neo4j connection is opened. If the first row is missing a required parameter, the run aborts with no connection attempted.
- **FR-007**: The tool MUST commit every 1000 rows by default during an `--apply` run.
- **FR-008**: The tool MUST accept `--batch <N>` (where N is an integer ≥ 1) to override the default commit interval; each completed batch of N rows is committed in its own transaction. `--batch 0` is an error caught before execution begins.
- **FR-009**: The tool MUST accept `--atomic` to wrap all `--apply` iterations in a single transaction; any error during the run rolls back the entire transaction.
- **FR-010**: `--batch <N>` and `--atomic` MUST be mutually exclusive; providing both is an error caught before execution begins.
- **FR-011**: On `--apply` failure, the tool MUST report the failing row index, the underlying Neo4j error, and—for batched runs—the number of rows committed in prior batches and the number rolled back in the current batch. For `--atomic` runs, the tool MUST report that the entire transaction was rolled back.
- **FR-012**: For `--apply` runs over more than one row, the tool MUST print progress lines to stderr so stdout (especially under `--json`) remains parseable. Cadence (per-row vs throttled per-N-rows) is implementation-defined; see Assumptions.
- **FR-013**: Under `--json` with `--apply`, stdout MUST be a JSON array of per-row result objects, each carrying a `"row"` index field in addition to the Milestone 1 result schema (`source`, `columns`, `rows`, optional `summary`).
- **FR-014**: CSV parsing MUST treat the first row as headers and every subsequent row as a data row. Empty header cells are an error.
- **FR-015**: JSONL parsing MUST be line-oriented: each non-empty line is parsed independently as a JSON object; a malformed line aborts the run with a line-number diagnostic.
- **FR-016**: JSON array parsing MUST require the top-level value to be an array and each element of the array to be a JSON object; any other top-level type, or any non-object array element, is an error.

- **FR-017**: When the data file contains zero data rows (CSV with only a header line, empty JSON array, empty JSONL file), the run MUST behave as follows: if the query has no required parameters, exit 0 silently without opening a Neo4j connection; if the query has at least one required parameter, exit 1 with an error identifying the empty data file ("no input rows found in <file>").

### Key Entities

- **Data File**: A CSV, JSON array, or JSONL file supplied to `--apply`; each row/object becomes one query execution.
- **Row Parameter Set**: The set of named parameters derived from a single data row, optionally merged with constant `--param` values.
- **Batch**: A contiguous group of rows committed as a single Neo4j transaction; size controlled by `--batch <N>` (default 1000) or collapsed to "all rows" by `--atomic`.
- **Per-Row Result**: One element of the `--json` output array, extending the Milestone 1 result schema with a `row` index field.
- **Progress Stream**: stderr output reporting per-row or per-batch advancement during an `--apply` run.

## Success Criteria *(mandatory)*

### Measurable Outcomes

- **SC-001**: A user can load a CSV, JSON array, or JSONL data file of arbitrary length into Neo4j by running a single `relate query <name> --apply <file> --write` command, with no intermediate format conversion required.
- **SC-002**: When a required parameter is missing from the first data row, the run aborts before any Neo4j connection is opened, and the error message identifies both the missing parameter and the data file as the source.
- **SC-003**: Under the default `--batch 1000` setting, a 10,000-row batch commits in 10 batches; under `--atomic`, the same batch commits exactly once. Verifiable by asserting post-failure row visibility: a mid-batch failure under `--batch N` leaves prior batches' rows visible; the same failure under `--atomic` leaves zero rows visible.
- **SC-004**: When a batch run fails on row M, the error message states the failing row index, the underlying Neo4j error, and exactly how many rows were committed in completed prior batches—no ambiguity about the partial-write state of the database.
- **SC-005**: `--apply --json` output is consumable by a standard JSON parser without preprocessing, even on multi-thousand-row runs where progress lines are simultaneously printed to stderr.
- **SC-006**: All `--apply` behaviors (each format, each transaction mode, each failure mode, each output mode) are exercised by the automated test suite; zero regressions in Milestone 1 or Milestone 2 behavior.

## Assumptions

- File format is detected solely by extension (`.csv`, `.json`, `.jsonl`); content-sniffing fallback is out of scope.
- JSONL is the recommended format for large datasets because it streams; JSON-array input is parsed in full before iteration begins, and very large JSON arrays may exhaust memory. This trade-off is documented but not mitigated in this milestone.
- CSV input has no native NULL representation; users who need to pass Cypher NULL values must use JSON or JSONL, or restructure their query to handle empty strings explicitly.
- The default `--batch 1000` matches the standard Neo4j bulk-load recommendation; users with unusually large rows or low memory budgets are expected to tune downward, and users seeking maximum throughput on small rows may tune upward.
- Preflight validation against the first row is representative: if the first row satisfies parameter requirements, the rest of the file is assumed to have the same shape. Per-row schema validation is out of scope; runtime errors will surface per row during execution.
- `--apply` with multi-statement query sources is supported in principle (each row runs all statements in order), but the precise error-reporting format for "statement K failed on row M" is left to the implementation and may be refined after first use.
- Constant parameters supplied via `--param` are merged into every row's parameter set with `--param` taking precedence on key conflicts—mirroring the precedence rules established in Milestones 1 and 2.
- Progress reporting cadence (per-row vs. per-batch) is an implementation choice; the only guaranteed behavior is that some progress signal appears on stderr for runs longer than one row.
