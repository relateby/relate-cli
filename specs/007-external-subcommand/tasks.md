# Tasks: External Subcommand Support

**Input**: Design documents from `/specs/007-external-subcommand/`
**Prerequisites**: plan.md ✅, spec.md ✅, research.md ✅, data-model.md ✅, contracts/ ✅

**Organization**: Tasks grouped by user story to enable independent implementation and testing.

## Format: `[ID] [P?] [Story?] Description`

- **[P]**: Can run in parallel (different files or non-overlapping code, no mutual dependency)
- **[Story]**: Which user story (US1, US2, US3)
- Exact file paths included in every description

---

## Phase 1: Setup (Shared Infrastructure)

**Purpose**: Wire clap's external-subcommand support and module declaration so the project compiles. Both tasks touch different files and can run in parallel.

- [ ] T001 [P] Add `#[command(external_subcommand)] External(Vec<String>)` as the final variant of the `Commands` enum in `src/cli.rs`
- [ ] T002 [P] Add `pub mod external;` declaration to `src/commands/mod.rs`

**Checkpoint**: `cargo check` passes — clap accepts the new variant, module is declared.

---

## Phase 2: Foundational (Blocking Prerequisites)

**Purpose**: Create the `external.rs` module with compilable stubs and connect it in `main.rs`. No user story work can begin until these compile.

**⚠️ CRITICAL**: Depends on T001 and T002. No user story work can begin until this phase is complete.

- [ ] T003 Create `src/commands/external.rs` with stub implementations: `pub fn exec_extension(name: &str, ext_args: &[String], neo4j: &crate::cli::Neo4jArgs) -> !` (body: `std::process::exit(0)`) and private `fn handle_exec_error(binary: &str, err: std::io::Error) -> !` (body: `std::process::exit(1)`)
- [ ] T004 Add `Commands::External(args)` match arm to the `match cli.command` block in `src/main.rs`: call `commands::external::exec_extension(name, ext_args, &cli.neo4j)` after splitting `args` with `split_first()`

**Checkpoint**: `cargo build` succeeds — binary compiles end-to-end with stub external dispatch.

---

## Phase 3: User Story 1 — Invoke a Peer Tool (Priority: P1) 🎯 MVP

**Goal**: `relate <name>` transparently delegates to `relate-<name>` on PATH, forwarding all remaining args and inheriting stdin/stdout/stderr.

**Independent Test**: Install a `relate-hello` stub script on PATH, run `relate hello world`, verify "hello world" appears in output and exit code is 0.

- [ ] T005 [US1] Implement `exec_extension()` body in `src/commands/external.rs`: `#[cfg(unix)]` block using `std::os::unix::process::CommandExt::exec()` (call `handle_exec_error()` on return); `#[cfg(not(unix))]` block using `std::process::Command::status()` + `std::process::exit(code)` (call `handle_exec_error()` on `Err`)
- [ ] T006 [US1] Add `make_stub_script(dir: &tempfile::TempDir, name: &str, body: &str)` helper and integration test `delegates_to_external_binary` in `tests/cli_integration.rs`: helper writes `#!/bin/sh\n{body}` to `{dir}/{name}` and calls `chmod +x`; test creates stub printing "hello from stub", prepends TempDir to PATH via `.env("PATH", ...)`, asserts `relate hello` stdout contains "hello from stub"
- [ ] T007 [P] [US1] Integration test `propagates_exit_code_zero` in `tests/cli_integration.rs`: stub exits 0, `relate hello` exits 0 (`.assert().success()`)
- [ ] T008 [P] [US1] Integration test `propagates_nonzero_exit_code` in `tests/cli_integration.rs`: stub body is `exit 42`, assert `relate hello` exits 42 (`.assert().code(42)`)
- [ ] T009 [US1] Integration test `builtin_takes_precedence_over_external` in `tests/cli_integration.rs`: place a `relate-lint` stub on PATH that prints "STUB", run `relate lint` with a valid .cypher fixture, assert stdout does NOT contain "STUB" and exit code is 0

**Checkpoint**: `cargo test` passes and all US1 tests are green. `relate hello` works with a manual stub.

---

## Phase 4: User Story 2 — Clear Error When Not Found (Priority: P2)

**Goal**: When `relate <name>` is called but `relate-<name>` is not on PATH, the user sees an actionable error naming the missing binary.

**Independent Test**: Run `relate nonexistent-subcommand`, verify stderr contains `relate-nonexistent` and exit code is 127.

- [ ] T010 [US2] Implement `handle_exec_error()` body in `src/commands/external.rs`: match on `err.kind()` — `ErrorKind::NotFound` → `eprintln!("error: external subcommand \`{binary}\` not found on PATH — install it to use \`relate {name}\`")` + `exit(127)`; `ErrorKind::PermissionDenied` → `eprintln!("error: external subcommand \`{binary}\` exists but is not executable — check file permissions")` + `exit(126)`; other → `eprintln!("error: failed to execute \`{binary}\`: {err}")` + `exit(1)`
- [ ] T011 [P] [US2] Integration test `not_found_exits_127_with_binary_name` in `tests/cli_integration.rs`: run `relate nonexistent-subcommand` with no TempDir on PATH, assert exit code 127 and stderr contains `relate-nonexistent`
- [ ] T012 [P] [US2] Integration test `not_executable_exits_126_with_message` in `tests/cli_integration.rs`: write a file named `relate-hello` to TempDir without execute permission (`0o644`), prepend TempDir to PATH, assert `relate hello` exits 126 and stderr contains "not executable"

**Checkpoint**: `cargo test` passes. Running `relate frobnicate` shows the not-found message naming `relate-frobnicate`.

---

## Phase 5: User Story 3 — Extension Receives Global Flags (Priority: P3)

**Goal**: Global flags (`--uri`, `--user`, `--password`) are forwarded to the extension as `RELATE_URI`, `RELATE_USER`, `RELATE_PASSWORD` environment variables.

**Independent Test**: Write a `relate-echo-env` stub that prints its environment. Run `relate --uri bolt://host:7687 echo-env`, verify `RELATE_URI=bolt://host:7687` in output.

- [ ] T013 [US3] Add env var forwarding to `exec_extension()` in `src/commands/external.rs` before the cfg blocks: `cmd.env("RELATE_URI", &neo4j.uri); cmd.env("RELATE_USER", &neo4j.user); if let Some(pw) = &neo4j.password { cmd.env("RELATE_PASSWORD", pw); }`
- [ ] T014 [P] [US3] Integration test `forwards_global_flags_as_env_vars` in `tests/cli_integration.rs`: stub body is `env | sort`, call `relate --uri bolt://host:7687 --user tester <name>`, assert stdout contains `RELATE_URI=bolt://host:7687` and `RELATE_USER=tester`
- [ ] T015 [P] [US3] Integration test `inherits_calling_environment_unchanged` in `tests/cli_integration.rs`: set `NEO4J_PASSWORD=secret` on the `Command` env before running `relate <name>`, stub prints env, assert `NEO4J_PASSWORD=secret` in output

**Checkpoint**: `cargo test` passes. `RELATE_URI`, `RELATE_USER`, and (when supplied) `RELATE_PASSWORD` appear in extension environment.

---

## Phase 6: Polish & Cross-Cutting Concerns

**Purpose**: Help text, agent discoverability, and verification that no regressions were introduced.

- [ ] T016 [P] Update `long_about` on the `Cli` struct in `src/cli.rs` to add: "Unknown subcommands are delegated to `relate-<name>` on PATH.\nExample: `relate csp solve foo.csp.gram` → executes `relate-csp solve foo.csp.gram`"
- [ ] T017 [P] Update `skills/relate/SKILL.md` routing table to add an external-dispatch entry explaining that unknown subcommands dispatch to `relate-<name>` binaries on PATH
- [ ] T018 Run `cargo clippy --all-targets` and resolve any warnings introduced by `src/commands/external.rs` or the `External` match arm in `src/main.rs`
- [ ] T019 Run `cargo test` to confirm all 9 new integration tests pass and all pre-existing built-in command tests remain green
- [ ] T020 Validate quickstart.md: create `relate-hello` shell script in `/tmp`, `chmod +x`, add `/tmp` to PATH, run `./target/debug/relate hello`, confirm output appears and exit code is 0

---

## Dependencies & Execution Order

### Phase Dependencies

- **Setup (Phase 1)**: No dependencies — T001 and T002 can start immediately in parallel
- **Foundational (Phase 2)**: Requires T001 and T002 — BLOCKS all user story work
- **US1 (Phase 3)**: Requires T003 and T004
  - T005 must complete before T006–T009 (exec path must exist before tests run)
  - T006 must complete before T007–T009 (test helper must exist)
  - T007 and T008 are parallel with each other
- **US2 (Phase 4)**: Requires T005 (exec path must be in place for handle_exec_error to be called)
  - T011 and T012 are parallel with each other
- **US3 (Phase 5)**: Requires T005 (exec path must be in place to forward env before exec)
  - T014 and T015 are parallel with each other
  - US3 is independent of US2 — can proceed after Phase 3
- **Polish (Phase 6)**: T016–T017 are parallel; T018–T020 require all implementation complete

### User Story Dependencies

- **US1 (P1)**: Depends on Foundational — no dependencies on US2 or US3
- **US2 (P2)**: Depends on US1 (exec path in place) — independent of US3
- **US3 (P3)**: Depends on US1 (exec path in place) — independent of US2; US2 and US3 can proceed in parallel after US1

### Parallel Opportunities

```
T001 ──┐
       ├──▶ T003 ──▶ T004 ──▶ T005 ──▶ T006 ──▶ T007 (parallel)
T002 ──┘                                          T008 (parallel)
                                                  T009
                                    T005 ──▶ T010 ──▶ T011 (parallel)
                                                       T012 (parallel)
                                    T005 ──▶ T013 ──▶ T014 (parallel)
                                                       T015 (parallel)
T016 (parallel with T017)
T018 ──▶ T019 ──▶ T020
```

---

## Implementation Strategy

### MVP First (User Story 1 Only)

1. Complete Phase 1: T001, T002
2. Complete Phase 2: T003, T004
3. Complete Phase 3: T005–T009
4. **STOP and VALIDATE**: `relate hello` works with a manual stub. All US1 tests pass.
5. Ship or demo the core delegation behavior

### Incremental Delivery

1. Phase 1 + 2 → compiles, dispatch is wired but stub
2. Phase 3 (US1) → MVP: `relate <name>` delegates transparently
3. Phase 4 (US2) → users see clear errors with binary name
4. Phase 5 (US3) → ecosystem extensions can use connection flags
5. Phase 6 → polish, help text, agent skills updated

### Parallel Team Strategy

With two developers after Phase 2:
- Developer A: US1 (Phase 3)
- Developer B: US1 tests (T007, T008, T009) — after T006 merges

After Phase 3:
- Developer A: US2 (Phase 4)
- Developer B: US3 (Phase 5) — US2 and US3 can proceed in parallel

---

## Notes

- [P] tasks = can run concurrently (non-overlapping code or different files)
- [USn] label = maps to user story for traceability
- Each checkpoint should be verified with `cargo build` or `cargo test` before proceeding
- Stub scripts for tests use `TempDir` — never pollute the system PATH permanently
- `handle_exec_error()` is stubbed in T003 and fully implemented in T010 — T003 just needs it to compile
- The `make_stub_script` helper (created in T006) is shared by US1, US2, and US3 test tasks
- Total: 20 tasks across 6 phases
