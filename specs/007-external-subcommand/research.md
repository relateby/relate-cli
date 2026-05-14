# Research: External Subcommand Support

**Branch**: `007-external-subcommand` | **Phase**: 0

## Decision Log

### 1. Clap 4 External Subcommand Dispatch

**Decision**: Use `#[command(external_subcommand)]` on a catch-all `Commands::External(Vec<String>)` variant.

**Rationale**: Clap 4's derive API has first-class support for this pattern. The variant receives `[subcommand_name, arg1, arg2, ...]` as a `Vec<String>`. It only fires when no other variant matches, so built-in subcommands naturally take precedence (FR-006). No structural changes to `Cli` are needed.

**Alternatives considered**:
- `allow_external_subcommands` at the struct level without a dedicated variant — less ergonomic and not the canonical derive-API approach.
- Intercept `clap::Error` on parse failure and retry — fragile; bypasses clap's help and error UX.
- Manual `std::env::args()` pre-parse — duplicates clap's logic and loses global-flag parsing.

---

### 2. Cross-Platform Process Replacement

**Decision**: On Unix (macOS, Linux), use `std::os::unix::process::CommandExt::exec()` for true process replacement. On non-Unix (Windows), spawn the child with `std::process::Command::status()` and forward the exit code via `std::process::exit()`.

**Rationale**:
- `exec()` satisfies FR-003 (replace own process) on the primary target platforms. The process image is replaced; no zombie or wrapper process is left behind.
- Windows does not have `execvp`. Spawning + forwarding the exit code is the idiomatic Windows fallback; the observable behavior (stdin/stdout/stderr pass-through, exit code forwarding) is identical from the user's perspective.
- No additional crate is needed — both code paths are in `std`.

**Alternatives considered**:
- `exec`-only crate (e.g., `exec`) — adds a dependency for functionality already in `std`.
- Always spawn+wait — loses true process replacement on Unix; violates FR-003 literally.
- Windows subsystem process creation APIs — overkill; `std::process::Command` handles this correctly.

---

### 3. Error Discrimination (Not Found vs. Permission Denied)

**Decision**: Inspect `std::io::ErrorKind` from the failed spawn/exec to distinguish:
- `ErrorKind::NotFound` → "external subcommand `relate-<name>` not found on PATH — install it to use `relate <name>`"
- `ErrorKind::PermissionDenied` → "external subcommand `relate-<name>` exists but is not executable — check file permissions"
- Other → "failed to execute `relate-<name>`: {err}"

**Rationale**: The spec's edge-case section explicitly requires different messages for missing vs. non-executable. Using `ErrorKind` avoids a separate PATH walk.

**Alternatives considered**:
- Pre-flight `which`/PATH search before exec — extra I/O and TOCTOU risk; OS error is authoritative.
- Single generic error message — fails the edge-case requirement.

---

### 4. Environment Variable Forwarding

**Decision**: Forward global connection flags as environment variables before exec:
- `--uri` → `RELATE_URI`
- `--user` → `RELATE_USER`
- `--password` → `RELATE_PASSWORD` (only when `Some`)

**Rationale**: Env vars are the standard inter-process communication for credentials and config in Unix pipelines. Extensions can read them without parsing `argv`. The password may already be `NEO4J_PASSWORD` in the environment; inheriting the process environment (default for `Command`) preserves it unchanged (FR-007, scenario 2).

**Alternatives considered**:
- Prepend flags to extension `argv` — extensions would need to parse flags identical to relate's global flags; creates tight coupling.
- Custom IPC channel — far too complex for the stated scope.

---

### 5. Code Location

**Decision**: New file `src/commands/external.rs` containing a single `pub fn exec_extension(name: &str, ext_args: &[String], neo4j: &Neo4jArgs) -> !` (on Unix) / `pub fn exec_extension(...) -> Result<()>` (cross-platform wrapper). Register it in `src/commands/mod.rs`.

**Rationale**: Follows the project convention of one file per subcommand handler. Keeps `main.rs` and `cli.rs` diffs minimal.

**Alternatives considered**:
- Inline in `main.rs` — violates the project convention and makes `main.rs` harder to read.
- Generic plugin module — over-engineered; no plugin registry exists in scope.
