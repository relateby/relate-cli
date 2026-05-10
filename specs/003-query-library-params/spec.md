# Feature Specification: Query Library and Ergonomic Parameters

**Feature Branch**: `003-query-library-params`  
**Created**: 2026-05-10  
**Status**: Draft  
**Input**: User description: "Milestone 2 — Query Library and Ergonomic Parameters as described in RFC-003"

## User Scenarios & Testing *(mandatory)*

### User Story 1 - Run a Named Query by Bare Name (Priority: P1)

A developer maintains a `./cypher/` directory with `.cypher` files as a query library. They want to invoke any query by its short name—`create_person`, `find_person`—without typing full file paths, as if the query files were callable functions.

**Why this priority**: The bare-name resolution is the core value proposition of Milestone 2. Without it, users still need full paths; everything else in this milestone builds on top of it.

**Independent Test**: Can be fully tested by placing a `.cypher` file in `./cypher/` and running `relate query <bare-name>`, verifying the correct query executes without any path syntax.

**Acceptance Scenarios**:

1. **Given** a file `./cypher/find_person.cypher` exists, **When** the user runs `relate query find_person`, **Then** the query in that file is executed.
2. **Given** no `.cypher` file matches the bare name, **When** the user runs `relate query unknown_query`, **Then** an error reports which directory was searched and that the query was not found.
3. **Given** a file at an explicit relative path `./scripts/adhoc.cypher`, **When** the user runs `relate query ./scripts/adhoc.cypher`, **Then** bare-name resolution is skipped and the explicit path is used directly.

---

### User Story 2 - Supply Parameters as an Inline Map Literal (Priority: P1)

A developer running a query wants to pass all parameters in a single positional argument using Cypher map syntax—`'{name: "Alice", age: 30}'`—instead of repeating `--param` for every key.

**Why this priority**: Inline map literals are the ergonomic companion to bare-name resolution. They make invocations feel like function calls and are tested together with Story 1.

**Independent Test**: Can be fully tested by running `relate query find_person '{name: "Alice"}'` and verifying the parameter is passed correctly to the query.

**Acceptance Scenarios**:

1. **Given** a query that uses `$name`, **When** the user provides `'{name: "Alice"}'` as the second positional argument, **Then** `$name` resolves to `"Alice"` at execution time.
2. **Given** both a positional map and `--param age=31`, **When** the user runs the query, **Then** `--param` values take precedence over map values for conflicting keys.
3. **Given** `--params params.json` and a positional map are both provided, **When** the user attempts to run the query, **Then** an error reports that these two input modes are mutually exclusive.
4. **Given** a positional map with JSON-style quoted keys `'{"name": "Alice"}'`, **When** the user runs the query, **Then** it is accepted and parsed identically to unquoted-key syntax.

---

### User Story 3 - Address a Named Statement Inside a Multi-Statement File (Priority: P2)

A developer has a multi-statement `.cypher` file (e.g., `./cypher/person.cypher`) containing multiple labelled queries. They want to execute a single named statement—`person/upsert` or `person/delete`—without running the whole file.

**Why this priority**: Statement-level addressing extends the library metaphor to large files, but requires cypherdoc (Story 4) to work. Depends on P1 bare-name resolution.

**Independent Test**: Can be fully tested by creating a two-statement file with cypherdoc name blocks and running `relate query person/upsert`, verifying only that statement executes.

**Acceptance Scenarios**:

1. **Given** `./cypher/person.cypher` has two cypherdoc-named statements `upsert` and `delete`, **When** the user runs `relate query person/upsert`, **Then** only the `upsert` statement is queued and executed.
2. **Given** the user runs `relate query person` (no slash), **When** the file contains multiple statements, **Then** all statements are queued and executed in order.
3. **Given** the user runs `relate query person/nonexistent`, **When** no statement with that name exists in the file, **Then** an error lists the addressable statement names found in the file.

---

### User Story 4 - Read Inline Documentation via `--describe` (Priority: P2)

A developer wants to understand what a query does—its purpose, required parameters, and return shape—without executing it. They use `relate query --describe <name>` to read the embedded cypherdoc documentation.

**Why this priority**: `--describe` depends on cypherdoc parsing. It is the primary way users discover the function library's API without reading raw `.cypher` files.

**Independent Test**: Can be fully tested by running `relate query --describe person` against a file with cypherdoc blocks and verifying the formatted documentation is printed without any query execution.

**Acceptance Scenarios**:

1. **Given** a `.cypher` file with a cypherdoc block, **When** the user runs `relate query --describe <name>`, **Then** the statement name, description, parameters, and return type are printed to stdout without executing the query.
2. **Given** a multi-statement file, **When** `--describe` is used, **Then** each named statement's documentation is shown in file order, clearly separated.
3. **Given** a `.cypher` file with no cypherdoc, **When** `--describe` is used, **Then** the raw query text is shown with a note that no documentation is present.
4. **Given** a missing required parameter at execution time, **When** preflight Stage 3 fails, **Then** the failing statement's cypherdoc is automatically appended to the error output as a usage hint.

---

### User Story 5 - Override the Query Directory (Priority: P3)

A developer whose query files live somewhere other than `./cypher/` wants to specify an alternate directory for bare-name resolution.

**Why this priority**: Edge case. The default `./cypher/` covers most projects; the override is a convenience for non-standard layouts.

**Independent Test**: Can be fully tested by placing a `.cypher` file in `./sql/` and running `relate query --cypher-dir ./sql find_person`, verifying the query is found.

**Acceptance Scenarios**:

1. **Given** `--cypher-dir ./queries` is provided, **When** a bare name is used, **Then** the alternate directory is searched instead of `./cypher/`.
2. **Given** the specified `--cypher-dir` directory does not exist, **When** a bare name is used, **Then** an error reports that the directory was not found.

---

### Edge Cases

- What happens when a bare name matches both a file `foo.cypher` and a directory `foo/` in the query directory?
- How does the system handle a `.cypher` file that is syntactically valid but has a malformed or incomplete cypherdoc block (e.g., missing closing `*/`)?
- What happens when a map literal parameter contains nested objects or lists—types that `--param` cannot express?
- How does statement-level addressing (`file/name`) behave when the file itself contains a `/` in the path and no `/`-separated query name is intended?
- What happens when two statements in the same file share the same cypherdoc name?

## Requirements *(mandatory)*

### Functional Requirements

- **FR-001**: The tool MUST resolve a bare query name (no path separator, no `.cypher` extension) to a `.cypher` file in the configured query directory (default: `./cypher/`).
- **FR-002**: The tool MUST accept an explicit file path as the query source, bypassing bare-name resolution entirely.
- **FR-003**: The tool MUST support a `file/statement` addressing form that executes a single named statement from a multi-statement file, where the statement name is the first non-empty line of its cypherdoc block.
- **FR-004**: The tool MUST accept a positional Cypher map literal (`[PARAMS]`) as the second positional argument, parsed as a `{ key: value, ... }` expression.
- **FR-005**: When both a positional map and `--param` flags are provided, `--param` values MUST take precedence over map values for conflicting keys.
- **FR-006**: The positional map and `--params <FILE>` MUST be mutually exclusive; providing both is an error.
- **FR-007**: The tool MUST parse cypherdoc (`/** ... */`) blocks from `.cypher` files, extracting statement name, description, `@param` declarations, and `@returns` tag.
- **FR-008**: The `--describe` flag MUST print cypherdoc documentation for all statements in the resolved query source without executing any query.
- **FR-009**: When a statement has cypherdoc and preflight Stage 3 reports a missing required parameter, the failing statement's cypherdoc MUST be appended to the error message.
- **FR-010**: The query directory MUST be overridable via `--cypher-dir <PATH>`.
- **FR-011**: When `file/statement` addressing is used and no matching statement name is found, the error MUST list addressable statement names present in the file.
- **FR-012**: Cypherdoc `@param` declarations MUST distinguish required parameters (`identifier`) from optional parameters with defaults (`[identifier=default]`), and preflight Stage 3 MUST use this distinction when checking provided parameters.

### Key Entities

- **Query Library**: A directory (`./cypher/` by default) containing named `.cypher` files treated as callable functions.
- **Bare Name**: A query identifier with no path separator and no `.cypher` extension, resolved against the query library.
- **Cypherdoc Block**: A `/** ... */` comment immediately preceding a Cypher statement, containing the statement name (first non-empty line), optional description, `@param` declarations, and an optional `@returns` tag.
- **Positional Map**: A Cypher map literal (`{ key: value, ... }`) passed as the second positional CLI argument, used to supply multiple parameters in a single argument.
- **Statement Address**: A `file/statement` string that identifies a single named statement within a multi-statement `.cypher` file.

## Success Criteria *(mandatory)*

### Measurable Outcomes

- **SC-001**: A user can invoke any named query in the `./cypher/` library using only a bare name and an optional inline parameter map, with no typing of file paths or extensions.
- **SC-002**: A user can discover the full API of a query file—parameter names, types, descriptions, and return shape—by running a single `--describe` command, without opening any file.
- **SC-003**: When a required parameter is omitted, the error message includes the cypherdoc documentation for the failing statement, so the user can correct the invocation without consulting external documentation.
- **SC-004**: Running `relate query --describe <name>` completes in under one second on any `.cypher` file up to 500 lines, with no Neo4j connection required.
- **SC-005**: All bare-name, map-literal, and `--describe` behaviors are exercised by the automated test suite; zero regressions in existing Milestone 1 behavior.

## Assumptions

- The default query directory is `./cypher/` relative to the current working directory; no project-level config file is introduced in this milestone (deferred to a future RFC).
- Cypherdoc parsing uses the `cypherdoc` sub-grammar already present in `tree-sitter-cypher 0.2`; no new grammar dependency is introduced.
- `--params <FILE>` (JSON file) and the positional `[PARAMS]` map are mutually exclusive; mixing them is an error surfaced before any query execution.
- Statement names within a single file are expected to be unique; behavior for duplicate names is undefined and left to a future lint rule.
- The `--cypher-dir` override is per-invocation only; a persistent project-level default is out of scope for this milestone.
- Type coercion for values in the positional map literal follows the same rules as `--param` for scalar types; complex types (lists, nested maps) are supported via the map literal's Cypher syntax.
