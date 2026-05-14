# Data Model: External Subcommand Support

**Branch**: `007-external-subcommand` | **Phase**: 1

## Entities

This feature introduces no persistent data structures. It is a dispatch mechanism operating on transient runtime values.

---

## Runtime Values

### ExternalDispatch

Represents a resolved request to invoke an external binary. Exists only for the duration of the dispatch call.

| Field | Type | Source | Notes |
|-------|------|--------|-------|
| `binary_name` | `String` | Derived: `"relate-".to_owned() + name` | The executable to find on PATH |
| `ext_args` | `Vec<String>` | From clap `External` variant, `[1..]` | All args after the subcommand name |
| `relate_uri` | `String` | `Neo4jArgs.uri` | Forwarded as `RELATE_URI` |
| `relate_user` | `String` | `Neo4jArgs.user` | Forwarded as `RELATE_USER` |
| `relate_password` | `Option<String>` | `Neo4jArgs.password` | Forwarded as `RELATE_PASSWORD` if `Some` |

### Error Cases

| Condition | `io::ErrorKind` | Exit Code | Message |
|-----------|-----------------|-----------|---------|
| Binary not on PATH | `NotFound` | 127 | `external subcommand \`relate-{name}\` not found on PATH — install it to use \`relate {name}\`` |
| Binary not executable | `PermissionDenied` | 126 | `external subcommand \`relate-{name}\` exists but is not executable — check file permissions` |
| Other OS error | any | 1 | `failed to execute \`relate-{name}\`: {err}` |

**Exit codes 126 and 127 are the POSIX conventions** for "command found but not executable" and "command not found" respectively. Using them makes `relate` behave consistently with shells.

---

## Clap AST Extension

The `Commands` enum gains one new variant:

```rust
#[command(external_subcommand)]
External(Vec<String>),
```

`Vec<String>` = `[subcommand_name, arg1, arg2, …]`. The `subcommand_name` element is `argv[1]` from the user's perspective; `ext_args` is `[1..]`.

---

## Invariants

1. Built-in subcommands (`lint`, `parse`, `query`, `write`, `read`, `mcp`) always match before `External`. Clap guarantees this by matching named variants first.
2. `External` is never reached when zero subcommand arguments are given — clap's required-subcommand constraint fires first and shows help.
3. The `binary_name` is always `relate-` prefixed — no user input is interpolated into a shell; the binary name is passed directly to `Command::new()`.
