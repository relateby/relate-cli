# Implementation Plan: External Subcommand Support

**Branch**: `007-external-subcommand` | **Date**: 2026-05-14 | **Spec**: [spec.md](spec.md)
**Input**: Feature specification from `/specs/007-external-subcommand/spec.md`

## Summary

Add transparent delegation to external `relate-<name>` binaries when an unknown subcommand is used, using clap 4's `external_subcommand` derive attribute and Unix `exec()` for process replacement. Global connection flags are forwarded as `RELATE_*` environment variables.

## Technical Context

**Language/Version**: Rust 1.85.0
**Primary Dependencies**: clap 4.6 (derive + env features), `std::process::Command`, `std::os::unix::process::CommandExt`
**Storage**: N/A
**Testing**: `cargo test`, `assert_cmd 2`, `predicates 3`, `tempfile 3`
**Target Platform**: macOS and Linux (primary — true exec via `CommandExt`); Windows (secondary — spawn + wait fallback)
**Project Type**: CLI (single binary)
**Performance Goals**: Extension lookup adds <5ms overhead (OS PATH resolution)
**Constraints**: No new runtime dependencies; use only `std` and existing clap
**Scale/Scope**: Small — 3 files modified or created, ~80 lines of new code

## Constitution Check

*GATE: Must pass before Phase 0 research. Re-checked after Phase 1 design.*

| Principle | Status | Notes |
|-----------|--------|-------|
| I. CLI-Friendly | ✅ Pass | Exit codes propagated exactly (POSIX 126/127 for exec errors); errors to stderr; no interactive prompts; stdin/stdout/stderr inherited by extension |
| II. Human-Readable | ✅ Pass | Error messages name the missing binary and include an install hint; distinct messages for not-found vs. not-executable |
| III. Agent-Friendly | ✅ Pass (exception noted) | External dispatch is infrastructure, not a new named command — no new `SKILL.md` required. Individual extensions (`relate-csp`, etc.) own their own skill files. The `skills/relate/SKILL.md` routing table will be updated to describe the dispatch mechanism. |
| IV. Self-Contained Help | ✅ Pass | The `--help` long_about on `Cli` will note that unknown subcommands are delegated to `relate-<name>` binaries |

**No violations requiring Complexity Tracking.**

## Project Structure

### Documentation (this feature)

```text
specs/007-external-subcommand/
├── plan.md              # This file
├── research.md          # Phase 0 output
├── data-model.md        # Phase 1 output
├── quickstart.md        # Phase 1 output
├── contracts/
│   └── external-dispatch.md   # Phase 1 output
└── tasks.md             # Phase 2 output (/speckit-tasks command)
```

### Source Code Changes

```text
src/
├── main.rs              # MODIFIED: add External arm to match
├── cli.rs               # MODIFIED: add External variant + update long_about
└── commands/
    ├── mod.rs           # MODIFIED: pub mod external
    └── external.rs      # NEW: exec_extension() implementation

skills/
└── relate/SKILL.md      # MODIFIED: add external-dispatch section to routing table

tests/
└── cli_integration.rs   # MODIFIED: new test module for external dispatch
```

**Structure Decision**: Single-project Rust CLI following existing project convention. All new logic in `commands/external.rs`; minimal diffs to `cli.rs` and `main.rs`.

---

## Phase 0: Research

See [research.md](research.md) for full decision log.

### Key Decisions

| Topic | Decision |
|-------|----------|
| Clap API | `#[command(external_subcommand)]` on `Commands::External(Vec<String>)` |
| Process replacement | `CommandExt::exec()` on Unix; `Command::status()` + `process::exit()` on Windows |
| Error discrimination | `io::ErrorKind::NotFound` → exit 127; `PermissionDenied` → exit 126 |
| Env var forwarding | `RELATE_URI`, `RELATE_USER`, `RELATE_PASSWORD` set before exec |
| Code location | `src/commands/external.rs` — matches project convention |

---

## Phase 1: Design

### 1. `src/cli.rs` changes

Add `External` variant to `Commands` enum:

```rust
#[command(external_subcommand)]
External(Vec<String>),
```

Update `Cli` struct `long_about` to include:
```
Unknown subcommands are delegated to `relate-<name>` on PATH.
Example: `relate csp solve foo.csp.gram` → executes `relate-csp solve foo.csp.gram`
```

### 2. `src/commands/external.rs` (new file)

```rust
use crate::cli::Neo4jArgs;
use std::process::Command;

pub fn exec_extension(name: &str, ext_args: &[String], neo4j: &Neo4jArgs) -> ! {
    let binary = format!("relate-{name}");
    let mut cmd = Command::new(&binary);
    cmd.args(ext_args);
    cmd.env("RELATE_URI", &neo4j.uri);
    cmd.env("RELATE_USER", &neo4j.user);
    if let Some(pw) = &neo4j.password {
        cmd.env("RELATE_PASSWORD", pw);
    }

    #[cfg(unix)]
    {
        use std::os::unix::process::CommandExt;
        let err = cmd.exec();
        // exec() only returns on failure
        handle_exec_error(&binary, err);
    }

    #[cfg(not(unix))]
    {
        match cmd.status() {
            Ok(status) => std::process::exit(status.code().unwrap_or(1)),
            Err(err) => handle_exec_error(&binary, err),
        }
    }
}

fn handle_exec_error(binary: &str, err: std::io::Error) -> ! {
    match err.kind() {
        std::io::ErrorKind::NotFound => {
            let name = binary.strip_prefix("relate-").unwrap_or(binary);
            eprintln!("error: external subcommand `{binary}` not found on PATH \
                       — install it to use `relate {name}`");
            std::process::exit(127);
        }
        std::io::ErrorKind::PermissionDenied => {
            eprintln!("error: external subcommand `{binary}` exists but is not executable \
                       — check file permissions");
            std::process::exit(126);
        }
        _ => {
            eprintln!("error: failed to execute `{binary}`: {err}");
            std::process::exit(1);
        }
    }
}
```

### 3. `src/main.rs` changes

Add arm to the `match cli.command`:

```rust
Commands::External(args) => {
    let (name, ext_args) = args.split_first()
        .expect("clap guarantees at least one element in External");
    commands::external::exec_extension(name, ext_args, &cli.neo4j);
}
```

### 4. `src/commands/mod.rs` changes

Add:
```rust
pub mod external;
```

### 5. Integration tests

New test module in `tests/cli_integration.rs` (or new file `tests/external_integration.rs`):

```rust
#[cfg(test)]
mod external_subcommand {
    use assert_cmd::Command;
    use predicates::str::contains;

    fn make_stub(name: &str, exit_code: i32) -> tempfile::TempDir {
        // write a shell script to a tempdir, chmod +x, prepend to PATH
    }

    #[test]
    fn delegates_to_external_binary() { ... }

    #[test]
    fn propagates_exit_code_zero() { ... }

    #[test]
    fn propagates_nonzero_exit_code() { ... }

    #[test]
    fn not_found_exits_127_with_binary_name_in_message() { ... }

    #[test]
    fn forwards_global_flags_as_env_vars() { ... }

    #[test]
    fn builtin_takes_precedence_over_external() { ... }
}
```

Tests use `tempfile::TempDir` to create stub scripts, prepend the tmpdir to `PATH` via `.env("PATH", ...)` on the `Command`, and assert on output and exit codes.

### 6. `skills/relate/SKILL.md` update

Add a section to the routing table explaining that unknown subcommands are dispatched to external binaries.

---

## Complexity Tracking

No constitution violations — table omitted.
