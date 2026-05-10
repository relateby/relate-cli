# Feature Specification: relate query — Single Query Execution

**Feature Branch**: `002-query-command`
**Created**: 2026-05-10
**Status**: Draft
**Input**: User description: "milestone 1 of @proposals/RFC-003-query-command.md to establish the `relate query` command for single query execution"

## User Scenarios & Testing *(mandatory)*

### User Story 1 - Run an inline Cypher query (Priority: P1)

A developer wants to quickly run a Cypher statement against Neo4j without creating a file,
using an inline expression from the command line.

**Why this priority**: The most immediate use case — ad-hoc queries during development and
debugging. Validates the core execution pipeline end-to-end with the simplest possible input.

**Independent Test**: Run `relate query -e "MATCH (n) RETURN count(n) AS total"` against a
live Neo4j instance and observe a result table with one row.

**Acceptance Scenarios**:

1. **Given** a running Neo4j instance and valid credentials, **When** the user runs `relate query -e "MATCH (n) RETURN count(n) AS total"`, **Then** the command prints a formatted result table and exits with code 0.
2. **Given** an inline statement with a syntax error, **When** the user runs `relate query -e "MATCH n RETURN"`, **Then** the command prints a lint diagnostic and exits with code 1 before connecting to Neo4j.

---

### User Story 2 - Run a Cypher file (Priority: P1)

A developer has a `.cypher` file (single or multi-statement) and wants to execute it
against Neo4j.

**Why this priority**: File-based execution is the primary workflow for repeatable,
versioned queries. Multi-statement files allow setup sequences to run atomically in order.

**Independent Test**: Create a `find.cypher` file with one MATCH statement, run
`relate query find.cypher`, and observe results.

**Acceptance Scenarios**:

1. **Given** a valid single-statement `.cypher` file, **When** the user runs `relate query find.cypher`, **Then** results are printed with the filename as the source header and the command exits with code 0.
2. **Given** a multi-statement `.cypher` file, **When** the user runs `relate query batch.cypher`, **Then** each statement executes in order, each result is printed with a `filename:line` source header, and a summary line is shown at the end.
3. **Given** a multi-statement file where the second statement has a lint error, **When** the user runs `relate query batch.cypher`, **Then** the command reports the lint error with source location and exits with code 1 before connecting to Neo4j.
4. **Given** a multi-statement file where the second statement fails at runtime, **When** the user runs `relate query batch.cypher`, **Then** the first statement's results are printed, the runtime error is reported, and the command exits with code 2.

---

### User Story 3 - Pass named parameters to a query (Priority: P2)

A developer runs a parameterized Cypher query and supplies values for `$x` parameters
via `--param` flags or a JSON file.

**Why this priority**: Parameterized queries are essential for safe, reusable statements.
Builds directly on P1 stories.

**Independent Test**: Run `relate query -e "MATCH (n:Person {name: $name}) RETURN n" --param name=Alice` and observe results filtered to the named person.

**Acceptance Scenarios**:

1. **Given** a query with `$name`, **When** the user passes `--param name=Alice`, **Then** the query executes with `name` bound to `"Alice"`.
2. **Given** a query with `$name` and `$age`, **When** the user passes `--params params.json` containing `{"name": "Alice", "age": 30}`, **Then** both parameters are bound correctly.
3. **Given** both `--param age=31` and `--params params.json` containing `"age": 30`, **When** the user runs the query, **Then** `age` resolves to `31` (`--param` takes precedence).
4. **Given** a query referencing `$name` and no `--param name` provided, **When** the user runs `relate query`, **Then** the command reports a missing required parameter error and exits with code 1 before connecting to Neo4j.
5. **Given** a `--param` value is provided for a parameter not referenced in the query, **When** the query runs, **Then** the command prints a warning but continues execution.

---

### User Story 4 - Allow write operations explicitly (Priority: P2)

A developer runs a Cypher statement that mutates the graph (CREATE, MERGE, SET, DELETE,
REMOVE, FOREACH) and must explicitly opt in with `--write`.

**Why this priority**: Safety rail that prevents accidental mutations. Requires P1 stories
to be in place first.

**Independent Test**: Run a MERGE statement without `--write`, observe a clear error;
re-run with `--write`, observe successful execution.

**Acceptance Scenarios**:

1. **Given** a statement containing `MERGE`, **When** the user runs without `--write`, **Then** the command prints an error naming the write clause and the source, suggests `--write`, and exits with code 1.
2. **Given** a statement containing `MERGE`, **When** the user runs with `--write`, **Then** the query executes and the command exits with code 0.
3. **Given** a multi-statement file where only the second statement contains a write clause, **When** the user runs without `--write`, **Then** the command reports the write violation on the second statement and exits before connecting.

---

### Edge Cases

- What happens when the `.cypher` file is empty? → Lint stage reports no statements found; exits code 1.
- What happens when Neo4j is unreachable? → Command exits with code 2 after preflight passes, with a connection error message.
- What happens when a query returns no rows? → Prints `(no rows returned)` with the source header; not an error.
- What happens when `-e` and a file path are both provided? → Error: mutually exclusive; exits code 1.
- What happens when `--params` file is not valid JSON? → Parse error reported; exits code 1 before connecting.
- What happens when `--param` value contains `=` (e.g., a URL)? → Only the first `=` is treated as the separator; remainder is the value.

## Requirements *(mandatory)*

### Functional Requirements

- **FR-001**: The command MUST accept an inline Cypher statement via `-e`/`--expr` (repeatable for multiple statements).
- **FR-002**: The command MUST accept a `.cypher` file path as `[QUERY]`; a multi-statement file produces one queue entry per statement.
- **FR-003**: `-e` and `[QUERY]` MUST be mutually exclusive; providing both is an error.
- **FR-004**: Before connecting to Neo4j, the command MUST lint all statements using the `cypher-data` engine; lint errors abort execution with exit code 1.
- **FR-005**: Before connecting to Neo4j, the command MUST classify each statement as Read or Write by inspecting the AST for write clauses (CREATE, MERGE, SET, DELETE, REMOVE, FOREACH).
- **FR-006**: If any statement is classified as Write and `--write` is not set, the command MUST exit with code 1 and a diagnostic naming the write clause and its source, before opening a Bolt connection.
- **FR-007**: Before connecting to Neo4j, the command MUST validate that all `$x` parameter references in the queue have a supplied value; missing required parameters abort execution with exit code 1.
- **FR-008**: Parameters MUST be suppliable via `--param NAME=VALUE` (repeatable) or `--params FILE` (JSON file); when both are provided, `--param` takes precedence on key conflicts.
- **FR-009**: `--param` values MUST be coerced: integer if fully numeric, float if numeric with decimal, `true`/`false` as boolean, otherwise string.
- **FR-010**: After preflight passes, the command MUST execute statements in queue order over a single Bolt connection; the first Neo4j error aborts remaining statements with exit code 2.
- **FR-011**: Results MUST be printed as a formatted table per statement, preceded by a source label (`filename`, `filename:line`, or `<inline>`).
- **FR-012**: Statements producing no rows MUST print `(no rows returned)` rather than an empty table; write statements MUST print an affected-nodes summary.
- **FR-013**: A summary line MUST be printed at the end: `N statements executed, M rows returned`.
- **FR-014**: With `--json`, output MUST be a JSON array of result objects (one per statement) containing `source`, `columns`, `rows`, and optionally `summary`.
- **FR-015**: The command MUST document in `--help` that lint is syntactic and runtime errors can still occur after lint passes.

### Key Entities

- **Statement Queue Entry**: An individual Cypher statement paired with its source label (file path + line, or `<inline>`).
- **Preflight Pipeline**: The ordered sequence of lint → write classification → parameter validation, all run before opening a Bolt connection.
- **Parameter Map**: The merged set of named values derived from `--param` flags and `--params` JSON, with `--param` taking precedence.

## Success Criteria *(mandatory)*

### Measurable Outcomes

- **SC-001**: A user can run an ad-hoc Cypher query against Neo4j in a single command with no file creation required.
- **SC-002**: A query with a syntax error is rejected with a human-readable diagnostic before any network connection is attempted, in under 1 second.
- **SC-003**: A write query run without `--write` is rejected with a clear, actionable error message before any network connection is attempted.
- **SC-004**: A query with a missing parameter is rejected with a clear error identifying the parameter name and its source location.
- **SC-005**: All statements in a multi-statement file execute in order; the first failure stops execution and the user can identify exactly which statement failed and why.
- **SC-006**: 100% of query results are presented in a readable table format with source attribution.

## Assumptions

- Users have a running Neo4j instance accessible via Bolt; connection credentials are supplied via existing global flags (`--uri`, `--user`, `--password` / `NEO4J_PASSWORD`).
- The `cypher-data` library (already used by `relate lint`) exposes the lint API needed by the preflight Stage 1.
- Write classification covers the clause set: CREATE, MERGE, SET, DELETE, REMOVE, FOREACH. CALL procedures are conservatively classified as Write unless in a known-safe allowlist (see RFC-003 Unresolved Questions).
- Milestone 1 does not include bare-name query library resolution (`./cypher/` directory), positional map parameters, cypherdoc parsing, or batch execution — those are Milestones 2 and 3.
- Single-crate project structure; the query command lives in `src/commands/query.rs`.
