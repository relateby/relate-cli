# CLI Contract: External Subcommand Dispatch

**Version**: 1.0 | **Feature**: 007-external-subcommand

## Overview

When `relate` receives a subcommand name that does not match any built-in (`lint`, `parse`, `query`, `write`, `read`, `mcp`), it searches PATH for a binary named `relate-<name>` and executes it, replacing the `relate` process.

---

## Invocation

```
relate [GLOBAL_FLAGS] <name> [ARGS...]
```

Where `<name>` is not a built-in subcommand.

**Example**:
```
relate --uri bolt://remote:7687 csp solve foo.csp.gram
```
Executes: `relate-csp solve foo.csp.gram` with `RELATE_URI=bolt://remote:7687`

---

## Binary Resolution

- Binary searched: `relate-<name>` (where `<name>` is the unknown subcommand)
- Search path: Standard system `PATH` — no additional directories added by `relate`
- Shell functions are not visible (follows `execvp` semantics)

---

## Argument Forwarding

All arguments that follow the subcommand name are forwarded verbatim as the extension's `argv`, in order.

```
relate csp solve foo.csp.gram
             ↓
relate-csp solve foo.csp.gram
```

---

## Environment Variables Set by `relate`

| Variable | Value | Notes |
|----------|-------|-------|
| `RELATE_URI` | Value of `--uri` flag | Default: `bolt://localhost:7687` |
| `RELATE_USER` | Value of `--user` flag | Default: `neo4j` |
| `RELATE_PASSWORD` | Value of `--password` flag | Only set when `--password` is provided |

The extension also inherits the full calling environment (including `NEO4J_PASSWORD` if set).

---

## Exit Codes

| Condition | Exit Code |
|-----------|-----------|
| Extension exits 0 | 0 |
| Extension exits N | N (propagated exactly) |
| `relate-<name>` not found on PATH | 127 |
| `relate-<name>` exists but not executable | 126 |
| Other execution failure | 1 |

---

## Error Messages

All error messages are written to stderr.

**Not found**:
```
error: external subcommand `relate-frobnicate` not found on PATH — install it to use `relate frobnicate`
```

**Not executable**:
```
error: external subcommand `relate-frobnicate` exists but is not executable — check file permissions
```

**Other failure**:
```
error: failed to execute `relate-frobnicate`: <OS error message>
```

---

## Help Text

`relate --help` includes a note that unknown subcommands are delegated to external binaries. Individual extensions are responsible for their own `--help` output.

---

## Precedence

Built-in subcommands always take precedence. If `relate-lint` is on PATH, `relate lint` still invokes the built-in lint command.

---

## Stdin / Stdout / Stderr

The extension inherits `relate`'s file descriptors directly (via exec on Unix, or inherited handles on Windows). There is no interception or buffering by `relate`.
