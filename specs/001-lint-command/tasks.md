# Tasks: relate lint Command

**Input**: Design documents from `specs/001-lint-command/`
**Prerequisites**: plan.md ✅, spec.md ✅, research.md ✅, data-model.md ✅, contracts/ ✅

**Organization**: Tasks are grouped by user story to enable independent implementation
and testing of each story. Tests are included per story for immediate validation.

## Format: `[ID] [P?] [Story] Description`

- **[P]**: Can run in parallel (different files, no shared state)
- **[Story]**: Which user story this task belongs to

---

## Phase 1: Setup

**Purpose**: Add dependencies and test fixtures. Nothing else can start until T001 succeeds.

- [x] T001 Add 8 new dependencies to Cargo.toml: cypher-data = "0.2.3", gram-data = "0.3.10", gram-diagnostics = "0.3.10", ariadne = "0.6", walkdir = "2", regex = "1", serde = { version = "1", features = ["derive"] }, serde_json = "1" — verify `cargo build` passes. Note: use `std::sync::LazyLock` for static regex (stable since Rust 1.85, matches rust-version); do NOT add once_cell.
- [ ] T002 [P] Create fixtures/valid.cypher and fixtures/invalid.cypher (use a known lint violation for invalid)
- [ ] T003 [P] Create fixtures/valid.gram and fixtures/invalid.gram (use a known lint violation for invalid)
- [ ] T004 [P] Create fixtures/doc_with_cypher.md (Markdown with a fenced cypher block containing a violation at a non-trivial line number, e.g. line 10+)
- [ ] T005 [P] Create fixtures/doc_with_gram.adoc (AsciiDoc with a [source,gram] block containing a violation)

---

## Phase 2: Foundational (Blocks All User Stories)

**Purpose**: Core types and CLI flag additions that every user story depends on.

**⚠️ CRITICAL**: No user story work can begin until this phase is complete.

- [ ] T006 Add `Lang` enum (variants: `Cypher`, `Gram`, implementing `clap::ValueEnum`) and `lang: Lang` field with `default_value_t = Lang::Cypher` to `LintArgs` in src/cli.rs — verify `relate lint --help` shows `--lang <LANG>  [default: cypher] [possible values: cypher, gram]`
- [ ] T007 [P] Define `LintDiagnostic { lang: Lang, source_file: Option<PathBuf>, inner: gram_diagnostics::Diagnostic }` and `Snippet { lang: Lang, source: String, fence_start_line: u32 }` in src/commands/lint.rs
- [ ] T008 [P] Define `JsonDiagnostic<'a>`, `JsonRange`, `JsonPosition` serialization structs in src/commands/lint.rs — `JsonPosition` uses field name `column` (rename from `gram_diagnostics::Position::character` via manual construction)

**Checkpoint**: Types compile, `--help` shows `--lang` — user story work can begin.

---

## Phase 3: User Stories 1 & 2 — Core File Linting (Priority: P1) 🎯 MVP

**Goal**: `relate lint file.cypher` and `relate lint file.gram` produce diagnostics with correct
location and exit codes.

**Independent Test (US1)**: `relate lint fixtures/invalid.cypher` exits 1 and prints file:line:col;
`relate lint fixtures/valid.cypher` exits 0.

**Independent Test (US2)**: `relate lint fixtures/invalid.gram` exits 1; `relate lint fixtures/valid.gram` exits 0.

### Implementation

- [ ] T009 Define `lint_path(path: &Path, strict: bool) -> anyhow::Result<Vec<LintDiagnostic>>` signature in src/commands/lint.rs — pass `strict: bool` directly; each dispatch branch constructs its own `LintOptions { strict }` inline (both engines use the same struct shape)
- [ ] T010 [P] [US1] Implement `.cypher` dispatch branch in `lint_path()` in src/commands/lint.rs: `let opts = cypher_data::lint::LintOptions { strict };` call `cypher_data::lint::lint_file(path, &opts)`, wrap results in `LintDiagnostic { lang: Lang::Cypher, source_file: Some(path.to_owned()), inner }`
- [ ] T011 [P] [US2] Implement `.gram` dispatch branch in `lint_path()` in src/commands/lint.rs: `let opts = gram_data::lint::LintOptions { strict };` call `gram_data::lint::lint_file(path, &opts)`, wrap results as `LintDiagnostic { lang: Lang::Gram, source_file: Some(path.to_owned()), inner }`
- [ ] T012 Add unsupported-extension arm to `lint_path()`: `_ => anyhow::bail!("unsupported file type: {}", path.display())`
- [ ] T013 Implement `print_human(diagnostics: &[LintDiagnostic], sources: &std::collections::HashMap<std::path::PathBuf, String>)` in src/commands/lint.rs using ariadne: group diagnostics by `source_file`; for each group look up source text from `sources`; build ariadne `Report` per diagnostic, convert `(line, character)` to byte offset by counting newlines then adding `character`; print via `Source::from(source_str)`
- [ ] T014 Wire `run(args: LintArgs) -> Result<()>` for the single-file case: collect `args.files`, for each path call `lint_path` and read the file source into a `HashMap<PathBuf, String>`, accumulate `LintDiagnostic`s, call `print_human(&diagnostics, &sources)`, use `std::process::exit(1)` if any diagnostics qualify (Error always; Warning/Hint/Info under `--strict`), propagate I/O errors with `?` (→ exit 2 via anyhow in main) in src/commands/lint.rs

### Tests

- [ ] T015 [P] [US1] Integration test `lint_cypher_clean_exits_zero`: `relate lint fixtures/valid.cypher` → success in tests/cli_integration.rs
- [ ] T016 [P] [US2] Integration test `lint_gram_clean_exits_zero`: `relate lint fixtures/valid.gram` → success in tests/cli_integration.rs
- [ ] T017 [P] [US1] Integration test `lint_cypher_violation_exits_one`: `relate lint fixtures/invalid.cypher` → exit 1, stdout contains filename and a line number in tests/cli_integration.rs
- [ ] T018 [P] [US2] Integration test `lint_gram_violation_exits_one`: `relate lint fixtures/invalid.gram` → exit 1 in tests/cli_integration.rs
- [ ] T019 [US1] Integration test `lint_unsupported_explicit_exits_two`: `relate lint fixtures/valid.cypher.txt` (or any unsupported extension given explicitly) → exit 2 in tests/cli_integration.rs

**Checkpoint**: All P1 tests pass — MVP is functional and independently deliverable.

---

## Phase 4: User Story 3 — Directory Walk (Priority: P2)

**Goal**: `relate lint <dir>` walks recursively, dispatches .cypher/.gram/.md/.adoc, skips other types.

**Independent Test**: `relate lint fixtures/` with a mix of supported and unsupported files exits 1
(violations present) and reports diagnostics from both .cypher and .gram files; .rs or .json files
produce no output and no error.

### Implementation

- [ ] T020 [US3] Implement `collect_paths(inputs: &[PathBuf]) -> anyhow::Result<Vec<PathBuf>>` in src/commands/lint.rs: for each input — if file, push directly (error on unsupported ext); if dir, use `WalkDir::new(dir).into_iter().filter_map(|e| e.ok())` filtering for extensions `cypher`, `gram`, `md`, `adoc`
- [ ] T021 [US3] Extend `run()` to use `collect_paths()` when `args.files` is non-empty and no `--expr`: replace direct per-file loop with collected paths in src/commands/lint.rs

### Tests

- [ ] T022 [US3] Integration test `lint_directory_reports_both_types`: `relate lint fixtures/` → exit 1, stdout mentions at least one .cypher diagnostic and one .gram diagnostic in tests/cli_integration.rs
- [ ] T023 [US3] Integration test `lint_directory_skips_unsupported`: create a temp dir with one valid .cypher and one .json file; `relate lint <tempdir>` → exit 0 (no violations from .cypher), no error about .json in tests/cli_integration.rs

**Checkpoint**: Directory linting works; US3 independently testable.

---

## Phase 5: User Story 4 — Inline Expression & Stdin (Priority: P2)

**Goal**: `relate lint --expr "MATCH (n) RETURN n"` lints an inline string; `relate lint` with no
arguments reads from stdin (FR-001 MUST). Both paths use `--lang` to select the engine.

**Independent Test (--expr)**: `relate lint --expr "MATCH (n) RETURN n"` exits 0;
`relate lint --lang gram --expr "(a)-[r]->(b)"` also exits 0.

**Independent Test (stdin)**: `echo "MATCH (n) RETURN n" | relate lint` exits 0;
`echo "<invalid>" | relate lint` exits 1.

### Implementation

- [ ] T024 [US4] Implement `--expr` path in `run()` in src/commands/lint.rs: if `args.expr` is Some, call `cypher_data::lint::lint_source` or `gram_data::lint::lint_source` based on `args.lang`, wrap results as `LintDiagnostic { source_file: None, ... }`, render and exit
- [ ] T048 [US4] Implement stdin path in `run()` in src/commands/lint.rs: when `args.files` is empty AND `args.expr` is None, read all of stdin via `std::io::Read::read_to_string`, lint the resulting string using the engine selected by `args.lang` (default: Cypher), wrap results as `LintDiagnostic { source_file: None, ... }`, render and exit

### Tests

- [ ] T025 [P] [US4] Integration test `lint_expr_cypher_valid`: `relate lint --expr "MATCH (n) RETURN n"` → exit 0 in tests/cli_integration.rs
- [ ] T026 [P] [US4] Integration test `lint_expr_gram_valid`: `relate lint --lang gram --expr "(a)-[r]->(b)"` → exit 0 in tests/cli_integration.rs
- [ ] T027 [US4] Integration test `lint_expr_invalid_exits_one`: `relate lint --expr` with a known-invalid expression → exit 1 in tests/cli_integration.rs
- [ ] T049 [US4] Integration test `lint_stdin_valid`: pipe a valid cypher expression to `relate lint` via stdin (using `assert_cmd`'s `.write_stdin()`) → exit 0 in tests/cli_integration.rs

**Checkpoint**: Inline expression and stdin linting both work independently.

---

## Phase 6: User Story 5 — Markdown/AsciiDoc Fence Extraction (Priority: P2)

**Goal**: `relate lint doc.md` lints embedded cypher/gram fences; diagnostics reference parent-doc line numbers.

**Independent Test**: `relate lint fixtures/doc_with_cypher.md` exits 1 and the reported line number
matches the actual line of the violation within the .md file (not line 1 of the snippet).

### Implementation

- [ ] T028 Compile static regex patterns using `std::sync::LazyLock` (stable since Rust 1.80, no extra dep needed) at module level in src/commands/lint.rs: Markdown pattern `` (?ms)^```[ \t]*(cypher|openCypher|gram)[ \t]*\n(.*?)^```[ \t]*$ `` and AsciiDoc pattern `(?ms)^\[source,[ \t]*(cypher|openCypher|gram)\]\n----\n(.*?)\n----`
- [ ] T029 Implement `extract_snippets(source: &str) -> Vec<Snippet>` in src/commands/lint.rs: apply both regex patterns, compute `fence_start_line` by counting `\n` chars before `match.start()`, return `Vec<Snippet>` sorted by line order
- [ ] T030 Implement `offset_diagnostic(inner: gram_diagnostics::Diagnostic, offset: u32) -> gram_diagnostics::Diagnostic` in src/commands/lint.rs: construct new `Diagnostic` with `range.start.line += offset` and `range.end.line += offset`
- [ ] T031 [US5] Add `.md`/`.adoc` dispatch branch to `lint_path()` in src/commands/lint.rs: read source, call `extract_snippets`, for each snippet call appropriate `lint_source`, apply `offset_diagnostic`, wrap as `LintDiagnostic { source_file: Some(path), ... }`

### Tests

- [ ] T032 [US5] Integration test `lint_markdown_line_offset`: `relate lint fixtures/doc_with_cypher.md` → exit 1, reported line ≥ fence start line (not 1) in tests/cli_integration.rs
- [ ] T033 [US5] Integration test `lint_markdown_non_cypher_fences_skipped`: .md with only ```python fences → exit 0, no output in tests/cli_integration.rs
- [ ] T034 [US5] Integration test `lint_asciidoc_fence_extraction`: `relate lint fixtures/doc_with_gram.adoc` → exit 1, reported line references parent .adoc file in tests/cli_integration.rs

**Checkpoint**: Markdown/AsciiDoc linting with correct line offsets works independently.

---

## Phase 7: User Story 6 — JSON Output (Priority: P2)

**Goal**: `relate lint --json` produces a valid JSON array matching the CLI contract schema.

**Independent Test**: `relate lint --json fixtures/valid.cypher` outputs `[]` and exits 0;
`relate lint --json fixtures/invalid.cypher` outputs a JSON array with at least one object
containing `severity`, `rule`, `message`, `file`, `range` fields.

### Implementation

- [ ] T035 [US6] Implement `print_json(diagnostics: &[LintDiagnostic])` in src/commands/lint.rs: map each `LintDiagnostic` to `JsonDiagnostic` (rename `character` → `column`, stringify severity, convert path to string), serialize with `serde_json::to_string_pretty`, print to stdout
- [ ] T036 [US6] Wire `--json` flag in `run()` in src/commands/lint.rs: call `print_json` instead of `print_human` when `args.json` is true

### Tests

- [ ] T037 [P] [US6] Integration test `lint_json_clean_outputs_empty_array`: `relate lint --json fixtures/valid.cypher` → exit 0, stdout is `[]` in tests/cli_integration.rs
- [ ] T038 [P] [US6] Integration test `lint_json_violation_outputs_array`: `relate lint --json fixtures/invalid.cypher` → exit 1, stdout parses as JSON array with object containing `"severity"`, `"rule"`, `"message"`, `"file"`, `"range"` keys in tests/cli_integration.rs

**Checkpoint**: JSON output works; CI pipelines can consume `relate lint --json`.

---

## Phase 8: User Story 7 — Strict Mode (Priority: P3)

**Goal**: `relate lint --strict` exits 1 on warnings, not just errors.

**Independent Test**: `relate lint --strict fixtures/warning-only.cypher` exits 1;
`relate lint fixtures/warning-only.cypher` exits 0.

### Implementation

- [ ] T039 [US7] Create fixtures/warning-only.cypher containing a Cypher construct that produces a warning but not an error (verify this produces a Warning-severity diagnostic via manual test first)
- [ ] T040 [US7] Implement strict exit-code logic in `run()` in src/commands/lint.rs: with `--strict`, exit 1 if any diagnostic has severity Warning, Information, or Hint in addition to Error; without `--strict`, exit 1 only on Error severity

### Tests

- [ ] T041 [P] [US7] Integration test `lint_strict_warning_exits_one`: `relate lint --strict fixtures/warning-only.cypher` → exit 1 in tests/cli_integration.rs
- [ ] T042 [P] [US7] Integration test `lint_no_strict_warning_exits_zero`: `relate lint fixtures/warning-only.cypher` (no --strict) → exit 0 in tests/cli_integration.rs

**Checkpoint**: Strict mode independently testable; all 7 user stories complete.

---

## Phase 9: Polish & Cross-Cutting Concerns

**Purpose**: Quality gates across all stories.

- [ ] T043 Verify `relate lint --help` includes: purpose, all flags with defaults, at least one usage example, and supported file types — update clap doc strings in src/cli.rs if any are missing
- [ ] T044 Run `cargo clippy --all-targets` and fix all warnings in src/commands/lint.rs and src/cli.rs
- [ ] T045 Run `cargo fmt --check` and format all changed files
- [ ] T046 Run full test suite `cargo test` and confirm all tests pass
- [ ] T050 Write skill description for `relate lint` at .claude/skills/relate-lint.md so agent frameworks can discover and invoke it (covers Constitution Principle III SHOULD: agent-discoverable)

---

## Dependencies & Execution Order

### Phase Dependencies

- **Setup (Phase 1)**: No dependencies — T002–T005 are parallel once T001 passes
- **Foundational (Phase 2)**: Depends on T001 (Cargo.toml) — BLOCKS all user story phases
- **US1+US2 (Phase 3)**: Depends on Phase 2 — MVP deliverable
- **US3 (Phase 4)**: Depends on Phase 3 (uses `lint_path`, `run` skeleton)
- **US4 (Phase 5)**: Depends on Phase 2 (uses types and Lang); can start in parallel with Phase 3. Covers both --expr and stdin (FR-001).
- **US5 (Phase 6)**: Depends on Phase 3 (uses `lint_path` and `lint_source` calls)
- **US6 (Phase 7)**: Depends on Phase 2 (uses `LintDiagnostic` and `JsonDiagnostic`); can start in parallel with Phase 3
- **US7 (Phase 8)**: Depends on Phase 3 (uses `run()` exit code logic)
- **Polish (Phase 9)**: Depends on all previous phases

### Parallel Opportunities Within Phases

- Phase 1: T002–T005 all parallel after T001
- Phase 2: T007–T008 parallel after T006
- Phase 3: T010–T011 parallel; T015–T019 mostly parallel
- Phase 5–7: Can be worked in parallel by different developers once Phase 3 is done

---

## Parallel Example: Phase 3 (MVP)

```
After T009 (lint_options helper):
  [parallel] T010 — cypher dispatch in lint_path
  [parallel] T011 — gram dispatch in lint_path
  ↓ (both done)
T013 — print_human with ariadne
T014 — wire run()
  ↓
[parallel] T015, T016, T017, T018, T019 — integration tests
```

---

## Implementation Strategy

### MVP First (User Stories 1 & 2 — Phase 3)

1. Complete Phase 1 (Setup) — T001 must pass before anything else
2. Complete Phase 2 (Foundational) — types and --lang flag
3. Complete Phase 3 (US1+US2) — full cypher and gram file linting with human output and exit codes
4. **STOP and VALIDATE**: Run `cargo test` and manually test `relate lint fixtures/`
5. Demo: `relate lint` on real .cypher and .gram files

### Incremental Delivery

- After Phase 3: MVP — single-file Cypher and Gram linting works
- After Phase 4: Directory walk — `relate lint .` works across a project
- After Phase 5: Inline linting — quick shell checks with `--expr`
- After Phase 6: Doc linting — Markdown/AsciiDoc embedded snippets validated
- After Phase 7: CI-ready — `--json` output for pipeline integration
- After Phase 8: Strict CI — `--strict` for zero-warning policy

---

## Notes

- `once_cell` is not needed if `std::sync::LazyLock` is used (stable since Rust 1.80, matches `rust-version` in Cargo.toml)
- ariadne's span parameter is a `std::ops::Range<usize>` (byte offsets); convert `(line, character)` by counting newlines in the source string up to the target line, then adding `character`
- `std::process::exit(1)` is the right mechanism for lint findings; `?` propagates tool errors (I/O, unsupported type) as exit 2 via anyhow in main
- Total tasks: 49 | MVP tasks (Phase 1–3): 19 | Remaining: 30
