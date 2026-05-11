# Tasks: Query Library and Ergonomic Parameters (Milestone 2)

**Input**: Design documents from `specs/003-query-library-params/`
**Prerequisites**: plan.md ✅, spec.md ✅, research.md ✅, data-model.md ✅, contracts/query-cli.md ✅

**Organization**: Tasks are grouped by user story. US1 + US2 are both P1 and
can be developed in parallel once the Foundational phase is complete. US3 and
US4 share the cypherdoc-parsing infrastructure and should be sequenced US3 → US4.
US5 reuses the `--cypher-dir` introduced by US1 and requires only tests.

## Format: `[ID] [P?] [Story] Description`

- **[P]**: Can run in parallel with other [P] tasks in the same phase (different files or non-conflicting sections)
- **[Story]**: Which user story this task belongs to
- All tasks include exact file paths

---

## Phase 1: Setup

**Purpose**: Add the one new crate dependency before any implementation begins.

- [x] T001 Add `tree-sitter-cypherdoc = "0.2"` to `[dependencies]` in `Cargo.toml`; run `cargo build` to verify the crate resolves and compiles

**Checkpoint**: `cargo build` succeeds with the new dependency.

---

## Phase 2: Foundational (Blocking Prerequisites)

**Purpose**: Structural changes shared by all user stories. Must be complete before
any story phase begins. All changes are additive or backward-compatible.

**⚠️ CRITICAL**: No user story work can begin until this phase is complete.

- [x] T002 Change `QueryArgs.query` field from `Option<PathBuf>` to `Option<String>` in `src/cli.rs`; update `build_queue_file()` call site(s) in `src/commands/query.rs` to convert with `Path::new(&s)` where a `&Path` is needed
- [x] T003 [P] Add three new fields to `QueryArgs` in `src/cli.rs`: `params_map: Option<String>` (second positional, after `query`); `describe: bool` (`--describe` flag); `cypher_dir: PathBuf` (`--cypher-dir`, default `"./cypher/"`)
- [x] T004 [P] Add `StatementSource::LibraryFile { path: PathBuf, bare_name: String }` and `StatementSource::LibraryStatement { path: PathBuf, stmt_name: String }` variants with `Display` impls to the `StatementSource` enum in `src/commands/query.rs`
- [x] T005 Add `CypherDoc` struct (`name`, `description`, `params: Vec<ParamDecl>`, `returns_raw`) and `ParamDecl` struct (`name`, `type_`, `required`, `default`, `description`) to `src/commands/query.rs`; add `doc: Option<CypherDoc>` field to `StatementEntry`; update all `StatementEntry { source, text }` construction sites to `StatementEntry { source, text, doc: None }`
- [x] T006 Add `QueryName` enum (`ExplicitPath(PathBuf)`, `BareName { name, resolved }`, `StmtAddress { file, stmt_name }`) and `resolve_query_source(query: &str, cypher_dir: &Path) -> QueryName` pure function to `src/commands/query.rs` (Decision 9 + 10 from research.md)

**Checkpoint**: `cargo test` passes; all existing tests green; new types compile without warnings.

---

## Phase 3: User Story 1 — Run a Named Query by Bare Name (Priority: P1)

**Goal**: `relate query find_person` resolves to `./cypher/find_person.cypher` and executes it.

**Independent Test**: Place a valid `MATCH (n) RETURN n` in `./cypher/find_person.cypher`, run
`relate query find_person`; verify the query executes. Run `relate query unknown` and verify
exit code 1 with a "not found" message naming the directory searched.

- [x] T007 [US1] Implement `open_library_file(name: &QueryName, cypher_dir: &Path) -> Result<Vec<StatementEntry>>` in `src/commands/query.rs` that handles `BareName` by opening `<cypher_dir>/<name>.cypher`; wire into `run()` to call it after `resolve_query_source()` when `args.query` is `Some` and not `--expr`
- [x] T008 [US1] Add "query not found" error path to `run()` in `src/commands/query.rs`: when the resolved file does not exist, print `Error: query '<name>' not found in <cypher_dir>  (looked for: <path>)` to stderr and `process::exit(1)`
- [x] T009 [US1] Add unit tests for `resolve_query_source()` covering: bare name → `BareName`; explicit path with separator → `ExplicitPath`; path ending `.cypher` → `ExplicitPath`; `file/stmt` form → `StmtAddress`; in `src/commands/query.rs` `#[cfg(test)]` module

**Checkpoint**: `relate query find_person` works end-to-end; `relate query missing` exits 1 with a
useful message; all unit tests pass.

---

## Phase 4: User Story 2 — Inline Map Literal Parameters (Priority: P1)

**Goal**: `relate query find_person '{name: "Alice"}'` passes `$name = "Alice"` to the query.

**Independent Test**: Run `relate query -e 'MATCH (n {name: $name}) RETURN n' '{name: "Alice"}'`
(no Neo4j required for unit tests); verify `$name` resolves to `"Alice"`. Run with both
`[PARAMS]` and `--params file` to verify mutual exclusion error (exit 1).

- [x] T010 [P] [US2] Implement `parse_map_literal(s: &str) -> Result<ParamMap>` in `src/commands/query.rs`: wrap input as `RETURN <s>`, parse with tree-sitter-cypher, walk the `map_literal` node, coerce values per Decision 8 (research.md)
- [x] T011 [US2] Update `build_param_map()` in `src/commands/query.rs`: load `args.params_map` via `parse_map_literal()` as the base layer (lowest precedence); add mutual exclusion check for `args.params_map.is_some() && args.params.is_some()` at the top of `run()` (exit 1 with message `Error: [PARAMS] and --params are mutually exclusive`)
- [x] T012 [US2] Add unit tests for `parse_map_literal()` in `src/commands/query.rs`: unquoted-key map; quoted-key map; integer/float/boolean/null values; invalid input → error; `--param` takes precedence over map on key conflict

**Checkpoint**: Map literal parameters work independently of bare-name resolution; mutual exclusion
tested; all unit tests pass.

---

## Phase 5: User Story 3 — Named Statement Addressing (Priority: P2)

**Goal**: `relate query person/upsert` executes only the statement named `upsert` from
`./cypher/person.cypher`.

**Independent Test**: Create `./cypher/person.cypher` with two cypherdoc-named statements
`upsert` and `delete`. Run `relate query person/upsert`; verify only the upsert statement
executes. Run `relate query person/nonexistent`; verify exit 1 with available names listed.

- [x] T013 [US3] Implement `parse_cypherdoc(raw: &str) -> Option<CypherDoc>` in `src/commands/query.rs` using `tree_sitter_cypherdoc::LANGUAGE`: parse the `document` root; extract `name`, `description` lines, `param_tag` nodes (field `param` → `required_param` or `optional_param`), and `returns_tag` raw text
- [x] T014 [US3] Update `build_queue_file()` in `src/commands/query.rs`: after splitting statements from the cypher AST, for each `statement` node look for a preceding `doc_comment` sibling; if found, call `parse_cypherdoc()` and store the result in `StatementEntry.doc`
- [x] T015 [US3] Implement `filter_by_stmt_name(entries: Vec<StatementEntry>, stmt_name: &str) -> Result<Vec<StatementEntry>>` in `src/commands/query.rs`: filter to entries whose `doc.as_ref().map(|d| d.name.as_str()) == Some(stmt_name)`; if none found, return an error listing available names; wire `StmtAddress` handling into `run()` after `open_library_file()`
- [x] T016 [US3] Update preflight Stage 3 in `src/commands/query.rs`: when `entry.doc` is `Some(doc)`, use `ParamDecl.required` to classify each declared param as required/optional instead of treating all `$x` refs as required; append the failing statement's cypherdoc block (name, description, params) to missing-parameter error messages
- [x] T017 [US3] Add unit tests in `src/commands/query.rs` `#[cfg(test)]` module: `parse_cypherdoc()` with a full block (name + description + required + optional params + returns); `parse_cypherdoc()` with name-only block; `filter_by_stmt_name()` hits; `filter_by_stmt_name()` miss lists available names

**Checkpoint**: `relate query person/upsert '{name: "Alice"}' --write` executes a single named
statement; missing required param includes cypherdoc in error; all unit tests pass.

---

## Phase 6: User Story 4 — `--describe` Documentation (Priority: P2)

**Goal**: `relate query --describe person` prints cypherdoc for all statements without executing.

**Independent Test**: Run `relate query --describe person` against a file with cypherdoc; verify
ruled-block format on stdout, no Bolt connection attempted, exit 0. Run against a file with no
cypherdoc; verify "(no documentation)" placeholder.

- [x] T018 [US4] Implement `print_describe(entries: &[StatementEntry])` in `src/commands/query.rs`: for each entry print a `──` ruler with the source label, the cypherdoc (name, description, @param lines, @returns if present), and the statement text indented 2 spaces; use "(no documentation)" when `doc` is `None`
- [x] T019 [US4] Add early-return branch in `run()` in `src/commands/query.rs`: after building the statement queue (source resolution + cypherdoc parsing) and before preflight Stage 1 (lint), if `args.describe` is true call `print_describe(&queue)` and `return Ok(())`; no Bolt connection is opened
- [x] T020 [US4] Add integration tests in `tests/` using `assert_cmd`: `--describe` against a file with two cypherdoc-named statements; verify stdout contains both names and param lines; verify exit 0; verify `--describe` with `--json` produces no JSON (describe is always human-readable)

**Checkpoint**: `--describe` exits 0 with formatted output; no connection required; integration
tests pass.

---

## Phase 7: User Story 5 — `--cypher-dir` Override (Priority: P3)

**Goal**: `relate query --cypher-dir ./queries find_person` resolves from `./queries/` instead
of `./cypher/`.

**Independent Test**: Place a `.cypher` file in a temp directory; run with `--cypher-dir <dir>`;
verify the query is found and executed.

- [x] T021 [US5] Add unit test for `resolve_query_source()` with a non-default `cypher_dir` path in `src/commands/query.rs`: verify `BareName.resolved` points into the overridden directory; verify "not found" error message names the overridden directory
- [x] T022 [US5] Add integration test in `tests/` using `assert_cmd` and `tempfile`: create a `.cypher` file in a temp directory; run `relate query --cypher-dir <tmpdir> <name>`; verify the query is resolved and executed (or fails with the correct error if no Neo4j is available)

**Checkpoint**: `--cypher-dir` override is validated; tests pass.

---

## Final Phase: Polish & Cross-Cutting Concerns

- [x] T023 [P] Update `--help` text in `QueryArgs` in `src/cli.rs`: update `long_about` to describe bare-name resolution and `file/stmt` addressing; update `after_help` examples to show `relate query create_person '{name: "Alice"}' --write`, `relate query person/upsert`, and `relate query --describe person`
- [x] T024 [P] Update `skills/relate-query/SKILL.md`: add M2 capabilities (bare-name resolution, `[PARAMS]` map literal, `file/stmt` addressing, `--describe`, `--cypher-dir`); update the workflow section and anti-rationalization table
- [x] T025 Run `cargo clippy --all-targets` and fix all warnings; run `cargo fmt --check` and apply formatting; run `cargo test` to confirm all tests pass

**Checkpoint**: `cargo clippy`, `cargo fmt --check`, and `cargo test` all pass with zero warnings.

---

## Dependencies & Execution Order

### Phase Dependencies

- **Setup (Phase 1)**: No dependencies — start immediately
- **Foundational (Phase 2)**: Depends on Phase 1
- **US1 (Phase 3)**: Depends on Foundational
- **US2 (Phase 4)**: Depends on Foundational; **independent of US1** (different code paths)
- **US3 (Phase 5)**: Depends on Foundational; depends on Phase 1 for `tree-sitter-cypherdoc`
- **US4 (Phase 6)**: Depends on US3 (reuses cypherdoc parsing)
- **US5 (Phase 7)**: Depends on US1 (reuses `cypher_dir` wiring); tests-only phase
- **Polish (Final)**: Depends on all story phases

### User Story Dependencies

```
Phase 1 (Setup)
  └─► Phase 2 (Foundational)
        ├─► Phase 3 (US1: bare-name) ──────────────────► Phase 7 (US5: cypher-dir)
        ├─► Phase 4 (US2: map literal)                               │
        └─► Phase 5 (US3: file/stmt) ──► Phase 6 (US4: describe)    │
                                                          └──────────┘
                                                    Phase 8 (Polish) ◄─ all stories
```

### Parallel Opportunities Within Foundational Phase

T003 (cli.rs additions) and T004 (query.rs StatementSource) touch different files and can run in parallel.

### Parallel Opportunities Across Stories

Once Phase 2 is complete, US1 (Phase 3) and US2 (Phase 4) can be developed in parallel — US1 focuses on query resolution in `run()`, US2 focuses on parameter building in `build_param_map()` / `parse_map_literal()`.

---

## Parallel Example: Foundational Phase

```
# These two tasks touch different files and can run simultaneously:
Task T003: "Add QueryArgs fields (params_map, describe, cypher_dir) in src/cli.rs"
Task T004: "Add LibraryFile/LibraryStatement variants to StatementSource in src/commands/query.rs"
```

## Parallel Example: US1 + US2

```
# After Phase 2 completes, these stories can start in parallel:
Developer A: Tasks T007 → T008 → T009 (bare-name resolution, query.rs)
Developer B: Tasks T010 → T011 → T012 (map literal parsing, query.rs)
# Note: both touch query.rs but at different functions; coordinate on merge
```

---

## Implementation Strategy

### MVP (US1 + US2 Only)

1. Complete Phase 1: Setup (T001)
2. Complete Phase 2: Foundational (T002–T006)
3. Complete Phase 3: US1 (T007–T009)
4. Complete Phase 4: US2 (T010–T012)
5. **STOP and VALIDATE**: `relate query find_person '{name: "Alice"}'` works end-to-end
6. Demo to stakeholders before proceeding to US3/US4

### Incremental Delivery

1. Setup + Foundational → shape of the API is established
2. US1 → bare-name queries work
3. US2 → map literal params work (can add on top of US1 or in parallel)
4. US3 → file/stmt addressing + cypherdoc-aware Stage 3
5. US4 → `--describe` (quick win after US3's cypherdoc parsing)
6. US5 → `--cypher-dir` tests (trivial, last)
7. Polish → help text + SKILL.md + linting

---

## Notes

- `[P]` within a phase = different files or non-conflicting sections; safe to parallelize
- `tree-sitter-cypherdoc` grammar root is `document`; statement name is the first named child with kind `name` (not a field — iterate named children)
- `doc_comment` nodes in the cypher AST contain the raw `/** ... */` text including delimiters; pass this text directly to the cypherdoc parser
- Map literal parsing wraps input as `RETURN <input>` to get a parseable Cypher statement; this reuses the existing cypher parser instance
- `--describe` is human-only output; no `--json` variant needed for M2
- Constitution III gate: `skills/relate-query/SKILL.md` **must** be updated (T024) before M2 is considered complete
