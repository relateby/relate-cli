---
number: "003"
title: "relate query — Parameterized Cypher Execution with Preflight Safety"
status: "Draft"
date: "2026-05-10"
authors:
  - "Andreas Kollegger <andreas.kollegger@neo4j.com>"
---

# RFC-003: relate query — Parameterized Cypher Execution with Preflight Safety

## Summary

`relate query` executes parameterized Cypher against Neo4j from the command line.
It treats `.cypher` files as a first-class query library, runs a preflight pipeline
(lint → write classification → parameter validation) before opening any Bolt
connection, and requires an explicit `--write` flag for mutations. A bare query name
resolves from a `./cypher/` directory, making a small collection of `.cypher` files
feel like a callable function library. Batch execution via `--apply` provides a
client-side equivalent of `LOAD CSV` that works without a server-accessible URL.

## Motivation

Developers working with Neo4j frequently need to run Cypher from scripts, templates,
or ad-hoc exploration. Existing options (cypher-shell, Neo4j Browser) are either
fully interactive or require a running desktop application. `relate query` fills the
scripting gap with a composable CLI designed around three use patterns:

**One-off queries** — run a Cypher statement inline or from a file during development
or debugging.

**CRUD query library** — a `./cypher/` directory with named `.cypher` files
(`create_person.cypher`, `find_person.cypher`, etc.) acts as a callable function
library. Each file is self-documenting via cypherdoc and callable by bare name:
`relate query create_person '{name: "Alice"}'`.

**Batch application** — apply a single parameterized query to many rows of data
(CSV, JSON, JSONL), the client-side equivalent of `LOAD CSV` without requiring
the data file to be at a URL reachable by the Neo4j server.

The design reflects two principles:

1. **Fail fast before connecting.** Lint errors, missing parameters, and unauthorized
   write operations are caught locally — no Bolt connection is opened until the
   preflight pipeline passes.

2. **Files as self-documenting query libraries.** Cypherdoc comments in `.cypher`
   files carry intent and parameter descriptions; `relate query` surfaces this
   documentation automatically rather than requiring a separate README.

## Design

### Full CLI Interface

The full interface across all milestones:

```
relate query [OPTIONS] [QUERY] [PARAMS]

Arguments:
  [QUERY]    A .cypher file path, a bare query name (resolved from ./cypher/),
             or omit when using -e for inline statements. Mutually exclusive with -e.
  [PARAMS]   Cypher map literal of named parameters, e.g. '{name: "Alice", age: 30}'
             Merged with --param flags; --param takes precedence on key conflicts.

Options:
  -e, --expr <EXPR>           Inline Cypher statement (repeatable; mutually exclusive
                              with [QUERY])
  -p, --param <NAME=VALUE>    Named parameter (repeatable)
      --params <FILE>         JSON file of named parameters
      --apply <FILE>          Apply query once per row in a CSV, JSON array, or JSONL
                              file; mutually exclusive with [PARAMS]
      --batch <N>             Rows per transaction for --apply [default: 1000]
                              Mutually exclusive with --atomic
      --atomic                Wrap all --apply iterations in a single transaction;
                              mutually exclusive with --batch
      --write                 Allow write operations (CREATE, MERGE, SET, DELETE, etc.)
      --describe              Print cypherdoc documentation without executing
      --json                  Output results as JSON
  -h, --help                  Print help

Connection options (global):
  --uri <URI>         Bolt URI [default: bolt://localhost:7687]
  --user <USER>       Neo4j username [default: neo4j]
  --password <PASS>   Neo4j password [env: NEO4J_PASSWORD]
```

The `--help` text includes:
> "Statements are linted before execution. Lint is syntactic — runtime errors
> (unknown labels, constraint violations) can still occur after lint passes."

---

## Milestone 1 — Single Query Execution

Core execution: one query source, preflight pipeline, results to stdout.

### Statement Queue

`relate query` builds an ordered queue of individual Cypher statements before
executing anything. A multi-statement `.cypher` file produces multiple queue
entries. The queue is always derived from a single source: either `[QUERY]`
(file path or bare name) or one or more `-e` flags. Mixing both is an error.

Each queue entry tracks its **source** for diagnostics and output headers:

| Input | Source label |
|-------|-------------|
| `-e "MATCH ..."` | `<inline>` |
| Single-statement `.cypher` file | `queries/find.cypher` |
| Statement N in multi-statement file | `queries/batch.cypher:<line>` |

Multi-statement files are split on statement boundaries by the tree-sitter-cypher
parser. Each statement becomes a separate queue entry.

### Preflight Pipeline

Before opening a Bolt connection, `relate query` runs the full queue through a
preflight pipeline. Each stage is fail-fast: the first error aborts the run with
a non-zero exit and a diagnostic pointing to the offending source.

**Stage 1 — Lint**

All statements are linted using the same `cypher-data` engine as `relate lint`.
Diagnostic format and exit-code conventions match `relate lint`. Lint is
syntactic and rule-based; it does not validate against a live schema.

**Stage 2 — Read/Write Classification**

Each statement is classified by inspecting the AST for write clauses:

| Clauses present | Classification |
|-----------------|---------------|
| MATCH, RETURN, WITH, UNWIND, CALL (read-only) | Read |
| CREATE, MERGE, SET, DELETE, REMOVE, FOREACH | Write |
| Mix of read and write clauses | Write |

If any statement is classified as **Write** and `--write` is not set, `relate
query` exits before connecting:

```
Error: write operation requires --write flag
  Statement: MERGE (n:Person {name: $name})
  Source: queries/upsert.cypher:5

  Re-run with --write to allow mutations.
```

**Stage 3 — Parameter Validation**

`relate query` is responsible for validating that provided parameters satisfy the
statement's requirements. The responsibility split with upstream tooling is:

| Concern | Owner |
|---------|-------|
| Param declared in cypherdoc but not referenced in statement | `cypher-data` lint rule |
| Param referenced in statement but not declared in cypherdoc | `cypher-data` lint rule |
| Provided params satisfy statement requirements at runtime | `relate query` Stage 3 |

Stage 3 proceeds in two steps:

**Step 1 — Determine required vs optional params.** Collect all `$x` references
from the statement AST. If the statement has cypherdoc param declarations, use them
to classify each param:
- Declared as `[identifier=default]` → optional; missing is a warning (the default applies)
- Declared as `identifier` (no brackets) → required; missing is an error
- Referenced in AST but absent from cypherdoc → treated as required

If no cypherdoc is present, all `$x` references are treated as required.

**Step 2 — Cross-check against provided params.** Any required param without a
provided value is a fatal error; any provided param with no `$x` reference in the
queue is a warning:

```
Error: missing required parameter '$name'
  Source: queries/upsert.cypher:5
  Hint: pass --param name=<value>
```

When cypherdoc is present for the failing statement, it is appended as a usage hint
(see [Cypherdoc](#cypherdoc) in Milestone 2).

### Named Parameters

Parameters are supplied via `--param` flags or a `--params` JSON file or both.
`--param` takes precedence over `--params` on key conflicts, mirroring Neo4j
Browser's `:param` / `:params` precedence.

```sh
# Scalar values
relate query -e "MATCH (n:Person {name: $name}) RETURN n" --param name=Alice

# Multiple params
relate query find.cypher --param name=Alice --param age=30

# JSON file for complex types (lists, maps, nested objects)
relate query find.cypher --params params.json
```

`--params` JSON format:

```json
{ "name": "Alice", "age": 30, "tags": ["dev", "admin"] }
```

`--param` type coercion: integer if fully numeric, float if numeric with a decimal
point, `true`/`false` as boolean, otherwise string. Use `--params` for any type
that cannot be expressed in this scheme (lists, maps, null).

### Execution

After preflight passes, `relate query` opens a single Bolt connection and executes
statements in queue order. Execution is fail-fast: the first Neo4j error aborts
the run. Results from already-executed statements are printed before the error.

Each statement's output is preceded by a source header:

```
-- queries/find.cypher
╭──────────┬─────╮
│ name     │ age │
├──────────┼─────┤
│ Alice    │ 30  │
╰──────────┴─────╯
1 row

-- queries/batch.cypher:3
(no rows returned)

2 statements executed, 1 row returned
```

Statements without result rows (e.g. `CREATE`, `MERGE`) print a summary of
affected nodes/relationships instead: `Created 1 node, set 2 properties`.

### Output

**Human-readable (default):** Unicode table per statement, preceded by source
label. Summary line at end.

**JSON (`--json`):** Array of result objects, one per statement:

```json
[
  {
    "source": "queries/find.cypher",
    "columns": ["name", "age"],
    "rows": [{ "name": "Alice", "age": 30 }]
  },
  {
    "source": "queries/batch.cypher:3",
    "columns": [],
    "rows": [],
    "summary": { "nodes_created": 1, "properties_set": 2 }
  }
]
```

### Exit Codes

| Code | Meaning |
|------|---------|
| 0 | All statements executed successfully |
| 1 | Preflight failure (lint error, missing parameter, write-without-flag) |
| 2 | Runtime failure (Neo4j connection error, query execution error) |
| 3 | `relate` internal error (I/O, parse failure) |

Preflight failures (code 1) are user-correctable. Runtime failures (code 2)
indicate environment or query logic issues. This split is useful for scripting.

---

## Milestone 2 — Query Library and Ergonomic Parameters

Bare-name resolution and positional params make a `./cypher/` directory feel like
a callable function library. Cypherdoc provides inline documentation.

### Query Library Resolution

When `[QUERY]` has no path separator and no `.cypher` extension, `relate query`
treats it as a bare query name and resolves it against a query directory.

**File-level resolution** — a bare name with no `/` resolves to a file:

```
Resolution order:
  1. <name>.cypher in the query directory (default: ./cypher/)
  2. Error: query '<name>' not found in ./cypher/
```

**Statement-level resolution** — a `file/statement` form targets a single named
statement within a multi-statement file. The statement name is the **first line**
of its cypherdoc block (see [Cypherdoc](#cypherdoc)):

```
Resolution order:
  1. <file>.cypher in the query directory
  2. Statement with @name matching <statement> in that file
  3. Error: statement '<statement>' not found in <file>.cypher
```

When `foo` resolves to a multi-statement file, all statements are queued in order.
When `foo/bar` is used, only the named statement is queued. `--describe foo` lists
all named statements in the file so the user can discover addressable sub-names.

The query directory can be overridden with `--cypher-dir <PATH>`. Project-level
defaults are out of scope for this milestone (see Unresolved Questions).

Examples:

```sh
# Resolves to ./cypher/create_person.cypher (single statement)
relate query create_person '{name: "Alice", home: "Cambridge, UK"}' --write

# Resolves to ./cypher/find_person.cypher
relate query find_person '{name: "Alice"}'

# Targets the statement named 'by_name' inside ./cypher/person.cypher
relate query person/by_name '{name: "Alice"}'

# List addressable statements in a multi-statement file
relate query --describe person

# Explicit path still works; no bare-name resolution
relate query ./scripts/adhoc.cypher
```

### Positional Parameters

The optional `[PARAMS]` positional argument is a Cypher map literal. It is
syntactically equivalent to a `{ key: value, ... }` map expression and parsed by
tree-sitter-cypher. Both unquoted and quoted keys are accepted:

```sh
# Unquoted keys (Cypher style)
relate query create_person '{name: "Alice", age: 30}' --write

# Quoted keys (JSON style — valid Cypher map syntax)
relate query create_person '{"name": "Alice", "age": 30}' --write
```

The positional map is merged with any `--param` flags. `--param` takes precedence
on key conflicts, allowing overrides of a base map:

```sh
relate query create_person '{name: "Alice", age: 30}' --param age=31 --write
# age is 31
```

`--params <FILE>` and `[PARAMS]` are mutually exclusive. Use `--params` when
parameters come from a file; use `[PARAMS]` for inline invocation.

### Cypherdoc

`.cypher` files may contain cypherdoc comments immediately preceding a statement.
Cypherdoc uses JSDoc-style `/** ... */` blocks with ` * ` line prefixes and is
recognized by the cypherdoc sub-grammar in `tree-sitter-cypher 0.2`.

**Block structure:**

```
/**
 * <name>
 *
 * <optional description>
 *
 * @param {type} identifier - Description
 * @param {type} [identifier=default] - Description (optional param)
 * @returns {[col: type][]} - Description
 */
```

The **first non-empty line** of the block is the statement name — there is no
`@name` tag. Only two tags exist in the grammar: `@param` and `@returns`.

Param identifiers in cypherdoc carry no `$` sigil; `relate query` maps `name`
to `$name` by convention. Required and optional params are distinguished by
bracket syntax:

| Syntax | Meaning |
|--------|---------|
| `@param {type} identifier - desc` | Required parameter |
| `@param {type} [identifier=default] - desc` | Optional; has a default value |

Type vocabulary: `string`, `integer`, `float`, `boolean`, `path`, `map`, `any`,
`node`, `relationship`, `list`, and parameterized forms like `node<Person>`,
`list<string>`.

The `@returns` tag uses a tuple type: `{[col: type]}` for one row,
`{[col: type][]}` for many rows.

Example multi-statement `./cypher/person.cypher`:

```cypher
/**
 * upsert
 *
 * Create or update a person node.
 *
 * @param {string} name - Unique name for the person
 * @param {string} [home=""] - Home city or region
 * @returns {[person: node<Person>]} - The upserted node
 */
MERGE (p:Person {name: $name})
SET p.home = $home
RETURN p

/**
 * delete
 *
 * Remove a person node by name.
 *
 * @param {string} name - Name of the person to remove
 */
MATCH (p:Person {name: $name})
DETACH DELETE p
```

These statements are addressable as `relate query person/upsert` and
`relate query person/delete`.

**`--describe` flag** — Print documentation for all statements in the query
without executing. For multi-statement files, each statement is shown with its
name, description, params, and statement text.

```
$ relate query --describe person

── person/upsert ──────────────────────────────────────────────────────
Create or update a person node.

@param {string} name          - Unique name for the person
@param {string} [home=""]     - Home city or region
@returns {[person: node<Person>]} - The upserted node

  MERGE (p:Person {name: $name}) SET p.home = $home RETURN p

── person/delete ──────────────────────────────────────────────────────
Remove a person node by name.

@param {string} name - Name of the person to remove

  MATCH (p:Person {name: $name}) DETACH DELETE p
```

**Auto-surface on missing parameters** — When preflight Stage 3 reports a missing
required parameter, the failing statement's cypherdoc is appended to the error:

```
Error: missing required parameter '$name'
  Source: ./cypher/person.cypher (person/upsert)
  Hint: pass --param name=<value>

  --- Documentation ---
  Create or update a person node.
  @param {string} name      - Unique name for the person
  @param {string} [home=""] - Home city or region
```

Only the failing statement's cypherdoc is shown, not the full file's.

---

## Milestone 3 — Batch Execution

`--apply` runs a single query once per row of a data file — the client-side
equivalent of `LOAD CSV` without requiring the file to be at a URL reachable
by the Neo4j server.

### `--apply`

```sh
# Apply to each row in a CSV
relate query create_person --apply people.csv --write

# Apply to each object in a JSON array
relate query create_person --apply people.json --write

# Apply to each line in a JSONL file
relate query create_person --apply people.jsonl --write
```

Supported formats:

| Format | Structure | Param mapping |
|--------|-----------|---------------|
| CSV | First row = headers; each subsequent row = one execution | header name → param name |
| JSON array | `[{...}, {...}]` | object key → param name |
| JSONL | One JSON object per line | object key → param name |

`--apply` is mutually exclusive with `[PARAMS]`. `--param` flags may still be
supplied alongside `--apply` to inject constant parameters across all rows.

The preflight pipeline runs once against the query with a synthetic parameter set
(all referenced `$x` satisfied by the first data row). If the first row is missing
a parameter, the run is aborted before any Neo4j connection is opened.

### Transaction Boundaries

`--apply` uses a three-tier transaction model:

| Mode | Behavior |
|------|----------|
| Default (`--batch 1000`) | Commit every 1000 rows |
| `--batch <N>` | Commit every N rows |
| `--atomic` | One transaction for all rows; full rollback on any error |

The default of 1000 matches the standard Neo4j bulk-load recommendation and avoids
the overhead of a commit-per-row on large inputs. Use `--batch 1` for per-row
commits when individual-row durability matters. `--batch` and `--atomic` are
mutually exclusive.

On failure, `relate query` stops at the failing row and reports how many rows were
committed in prior batches:

```
# Default / --batch N
Error on row 1012: Neo.ClientError.Schema.ConstraintValidationFailed
  1000 rows committed (batch 1), 11 rows in current batch rolled back.

# --atomic
Error on row 1012: Neo.ClientError.Schema.ConstraintValidationFailed
  Transaction rolled back. 0 rows committed.
```

### Progress Reporting

For batches larger than one row, `relate query` prints progress to stderr so it
does not interfere with `--json` stdout:

```
[11/50] applying row 11...
Error on row 12: ...
```

With `--json`, the stdout result is an array of per-row result objects in the same
schema as Milestone 1 output, with an added `"row"` index field.

---

## Unresolved Questions

- **Upstream lint rules for cypherdoc/statement mismatch:** `cypher-data` should provide
  lint rules flagging params declared in cypherdoc but not referenced as `$identifier` in
  the statement, and vice versa. These rules do not exist yet and represent a dependency
  for full Stage 3 accuracy. Until they land, `relate` cross-checks at runtime (Stage 3)
  but cannot catch documentation/statement mismatches at lint time (Stage 1).

- **`CALL` procedure classification:** Some procedures are read-only, others write.
  The AST alone cannot distinguish them. Current plan: classify any `CALL` not in
  a known-safe allowlist as Write. A `--trust-call` escape hatch may be needed.

- **`--param` null handling:** `--param value=null` is ambiguous (string "null" vs.
  Cypher NULL). Proposal: treat `null` as Cypher NULL only in `--params` JSON and
  the positional map literal, never from `--param` flag values.

- **Project-level query directory config:** `--cypher-dir` is per-invocation. A
  project-level default (`.relaterc`, `relate.toml`, or `Cargo.toml` metadata) would
  make bare-name resolution automatic. Format and location are unresolved.

- **Dry-run mode:** A `--dry-run` flag (show preflight results without connecting)
  may be useful for CI validation. Deferred.

- **Output format beyond `--json`:** `--format csv|tsv` for piping results into
  other tools. Deferred; `--json` + `jq` covers the machine-readable case for now.

- **`--apply` with multi-statement queries:** If the query source contains multiple
  statements, each row runs all statements in order. With `--atomic`, all statements
  across all rows share one transaction. The semantics are clear but the error
  reporting (which statement, which row) needs care.

- **Streaming large `--apply` inputs:** JSON array format requires parsing the full
  file before iteration. JSONL streams naturally. For large datasets, JSONL should
  be the recommended format; this should be documented.
