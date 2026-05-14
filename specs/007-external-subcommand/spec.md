# Feature Specification: External Subcommand Support

**Feature Branch**: `007-external-subcommand`
**Created**: 2026-05-14
**Status**: Draft
**Input**: User description: "external subcommand support for `relate` as described above"

## User Scenarios & Testing *(mandatory)*

### User Story 1 - Invoke a peer tool via `relate` (Priority: P1)

A developer has installed `relate-csp` (a constraint-solver tool) alongside `relate`. Rather than learning a separate entry point, they run `relate csp solve foo.csp.gram` and the peer tool executes transparently, receiving the remaining arguments.

**Why this priority**: This is the core value — a unified entry point for the `relate` ecosystem. Without it the feature has no purpose.

**Independent Test**: Install a `relate-hello` stub script on PATH. Run `relate hello`. Verify the stub executes and its output reaches the user.

**Acceptance Scenarios**:

1. **Given** `relate-csp` is on PATH, **When** the user runs `relate csp solve foo.csp.gram`, **Then** `relate-csp` is executed with arguments `solve foo.csp.gram` and its output is shown to the user.
2. **Given** `relate-csp` exits with code 0, **When** `relate csp ...` finishes, **Then** `relate` also exits with code 0.
3. **Given** `relate-csp` exits with a non-zero code, **When** `relate csp ...` finishes, **Then** `relate` propagates that exact exit code.

---

### User Story 2 - Clear error when no extension is found (Priority: P2)

A user types `relate frobnicate` expecting it to delegate, but no `relate-frobnicate` binary exists on PATH.

**Why this priority**: Silent failure or a confusing clap error would harm discoverability. A clear message tells the user what was attempted.

**Independent Test**: Run `relate nonexistent-subcommand`. Verify the error message names the binary that was searched for.

**Acceptance Scenarios**:

1. **Given** `relate-frobnicate` is not on PATH, **When** the user runs `relate frobnicate`, **Then** `relate` exits non-zero and prints a message indicating `relate-frobnicate` was not found.
2. **Given** the unknown subcommand name, **When** the error is displayed, **Then** the message includes the expected binary name (`relate-frobnicate`) so the user knows what to install.

---

### User Story 3 - Extension receives global flags (Priority: P3)

A user runs `relate --uri bolt://remote:7687 csp solve foo.csp.gram`. The extension binary can read the Neo4j connection flags forwarded as environment variables or arguments so it can connect to the same database.

**Why this priority**: Useful for a cohesive ecosystem experience, but the extension can always accept its own flags; this is an enhancement.

**Independent Test**: Write a `relate-echo` stub that prints its argv and environment. Run `relate --uri bolt://host:7687 echo`. Verify the URI value is accessible to the stub.

**Acceptance Scenarios**:

1. **Given** global flags (`--uri`, `--user`, `--password`) are supplied before the subcommand, **When** `relate` delegates to the extension, **Then** the flags are passed as `RELATE_URI`, `RELATE_USER`, and `RELATE_PASSWORD` environment variables.
2. **Given** `NEO4J_PASSWORD` is set in the calling environment, **When** delegation occurs, **Then** the extension inherits it unchanged.

---

### Edge Cases

- What happens when a `relate-<name>` binary exists but is not executable? `relate` should report a permission error, not a "not found" error.
- What happens when the subcommand name collides with a future built-in? Built-in subcommands always take precedence over external ones.
- What happens when no subcommand is provided at all? Standard clap "missing subcommand" help text is shown; no extension lookup is attempted.
- What if `relate-<name>` is a shell function rather than a file on PATH? Behavior follows the OS `execvp` semantics — shell functions are not visible.

## Requirements *(mandatory)*

### Functional Requirements

- **FR-001**: The CLI MUST attempt to locate and execute `relate-<name>` on PATH when `<name>` is not a recognized built-in subcommand.
- **FR-002**: The extension binary MUST receive all arguments that followed the subcommand name, in order, as its `argv`.
- **FR-003**: The CLI MUST replace its own process with the extension (exec-style), so the extension's stdin/stdout/stderr are connected directly to the terminal.
- **FR-004**: The CLI MUST propagate the extension's exit code as its own exit code.
- **FR-005**: The CLI MUST print a clear, actionable error message and exit non-zero when the expected `relate-<name>` binary is not found on PATH.
- **FR-006**: Built-in subcommands (`lint`, `parse`, `query`, `write`, `read`, `mcp`) MUST always take precedence over any external binary with the same name.
- **FR-007**: Global connection flags (`--uri`, `--user`, `--password`) MUST be forwarded to the extension as environment variables (`RELATE_URI`, `RELATE_USER`, `RELATE_PASSWORD`).
- **FR-008**: The extension lookup MUST use the standard system PATH, with no additional search paths added by `relate`.

## Success Criteria *(mandatory)*

### Measurable Outcomes

- **SC-001**: A peer tool installed as `relate-<name>` on PATH is reachable via `relate <name>` with no additional configuration.
- **SC-002**: The full output and exit code of an extension are indistinguishable from running the extension directly.
- **SC-003**: Users receive a message naming the missing binary within one second when an unknown subcommand is entered.
- **SC-004**: Zero regressions in existing built-in subcommand behavior as measured by the current test suite.

## Assumptions

- Extensions are standalone executables already on the user's PATH; `relate` takes no responsibility for installing or locating them elsewhere.
- The `exec`-style replacement is acceptable on all target platforms (macOS, Linux, Windows via WSL or native).
- Password forwarding via environment variable is acceptable given the password may already be in the environment (`NEO4J_PASSWORD`); no additional secrets-handling is introduced.
- There is no discovery mechanism (e.g., `relate extensions list`) in this feature; that is out of scope.
