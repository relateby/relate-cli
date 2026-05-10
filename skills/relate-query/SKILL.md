---
name: relate-query
description: >
  Execute parameterized Cypher statements against Neo4j using `relate query`.
  Use this skill when you need to run Cypher from the command line — inline,
  from a .cypher file, or by bare name from a query library — with preflight
  linting, write protection, named parameter support, and inline documentation.
  Covers single statements, multi-statement files, query libraries, file/stmt
  addressing, --describe documentation, and JSON output.
triggers:
  - "relate query"
  - "run cypher"
  - "execute cypher"
  - "query neo4j"
  - "cypher query"
  - "parameterized cypher"
  - "send query to neo4j"
  - "query library"
  - "cypher library"
  - "describe query"
---

# Skill: relate query

> **Purpose**: Execute Cypher statements against Neo4j with preflight safety.
> All statements are linted before execution. Write operations require `--write`.

---

## Step 1 — Choose input mode

| Situation | Command |
|-----------|---------|
| Quick inline statement | `relate query -e "MATCH (n:Person) RETURN n.name"` |
| Multiple inline statements | `relate query -e "STMT1" -e "STMT2"` |
| Single `.cypher` file | `relate query queries/find_person.cypher` |
| Bare query name (library) | `relate query find_person` |
| Named statement in a file | `relate query person/upsert` |
| Multi-statement `.cypher` file | `relate query queries/setup.cypher` |

`-e` and a file path are **mutually exclusive** — use one or the other.

**Bare names** (no path separator, no `.cypher` extension) are resolved from the
query library directory (default: `./cypher/`). Use `--cypher-dir <path>` to
override.

**`file/stmt` form** targets a single named statement within a multi-statement
`.cypher` file. The statement name is the first word of its cypherdoc block.

**Checkpoint**: You know whether to use `-e`, a file path, a bare name, or `file/stmt`.

---

## Step 2 — Supply parameters

Cypher statements may reference `$name`-style parameters. Supply values with:

| Situation | Flag |
|-----------|------|
| Single scalar value | `--param name=Alice` |
| Multiple scalars | `--param name=Alice --param age=30` |
| Inline map literal | `'{name: "Alice", age: 30}'` (second positional argument) |
| Complex types (lists, maps) or many params | `--params params.json` |
| Mix (map as base, flag overrides) | `'{name: "Alice"}' --param name=Bob` |

`--param` always takes precedence over the positional map on key conflicts.
The positional map `[PARAMS]` and `--params <FILE>` are **mutually exclusive**.

Type coercion for `--param` and map literal values:
- Fully numeric → integer (`30` → `30`)
- Numeric with decimal → float (`3.14` → `3.14`)
- `true` / `false` → boolean
- `null` in map literal → null
- Anything else → string

**Note**: Cypher map literal keys must be unquoted identifiers (`{name: "Alice"}`).
JSON-style quoted keys (`{"name": "Alice"}`) are not valid Cypher map syntax.
For quoted-key JSON, use `--params file.json` instead.

**Checkpoint**: All `$parameter` references in your statement have supplied values.
Missing params abort with exit code 1 before any connection is attempted.

---

## Step 3 — Declare write intent

`relate query` classifies each statement before connecting. Any statement containing
`CREATE`, `MERGE`, `SET`, `DELETE`, `REMOVE`, or `FOREACH` is a write operation and
requires `--write`:

```bash
# Read — no flag needed
relate query -e "MATCH (n:Person) RETURN n.name"

# Write — must add --write
relate query -e "MERGE (n:Person {name: $name})" --param name=Alice --write

# Library query with write
relate query create_person '{name: "Alice", age: 30}' --write

# file/stmt write
relate query person/upsert '{name: "Alice"}' --write
```

Omitting `--write` on a write statement exits with code 1 and names the offending
clause and source — before any Bolt connection is opened.

**Checkpoint**: You have added `--write` if and only if the statement mutates the graph.

---

## Step 4 — Discover query documentation with `--describe`

Use `--describe` to print inline cypherdoc documentation for any query without
executing it. No Neo4j connection is required.

```bash
# Describe all statements in a library file
relate query --describe person

# Describe a single named statement
relate query --describe person/upsert

# Describe an explicit file
relate query --describe queries/setup.cypher
```

Output format per statement:
```
── ./cypher/person.cypher (upsert) ─────────────────────────────────────
Create or update a person node.

@param {string} name - Unique name for the person
@param {string} [home=""] - Home city or region
@returns {[person: node<Person>][]} - The upserted node

  MERGE (p:Person {name: $name}) SET p.home = $home RETURN p
```

For files with no cypherdoc, `(no documentation)` is shown with the raw statement text.

`--describe` exits 0 and never opens a Bolt connection.

**Checkpoint**: You know the required parameters and write intent before running.

---

## Step 5 — Choose output format

- **Human-readable table** (default): results printed per statement with source header
- **JSON** (`--json`): stable array schema, suitable for piping or scripting

```bash
# Human-readable
relate query -e "MATCH (n:Person) RETURN n.name AS name, n.age AS age"

# JSON for scripting
relate query -e "MATCH (n) RETURN count(n) AS total" --json | jq '.[0].rows[0].total'
```

JSON output schema per statement:
```json
{
  "source": "<inline>",
  "columns": ["name", "age"],
  "rows": [{"name": "Alice", "age": 30}],
  "summary": {
    "nodes_created": 0, "nodes_deleted": 0,
    "relationships_created": 0, "relationships_deleted": 0,
    "properties_set": 0, "labels_added": 0
  }
}
```

**Checkpoint**: You have decided whether downstream processing needs `--json`.

---

## Step 6 — Run and interpret exit code

| Exit code | Meaning | Next action |
|-----------|---------|-------------|
| `0` | All statements executed (or `--describe` printed) | Done |
| `1` | Preflight failure | Read stderr — lint error, missing param, write-without-flag, query not found |
| `2` | Runtime failure | Neo4j connection refused or query execution error |
| `≥3` | Internal error | I/O or parse failure; check the file path |

**Checkpoint**: Exit code is captured. If non-zero, act on the stderr diagnostic.

---

## Step 7 — Multi-statement files

A `.cypher` file may contain multiple statements. **Statements must be separated
by semicolons (`;`)** — the tree-sitter-cypher parser uses `;` as the statement
boundary. `relate query` executes them in order, fail-fast.

```cypher
/** find_all */
MATCH (n:Person) RETURN n.name AS name;

/** upsert
 * @param {string} name - Person name
 */
MERGE (p:Person {name: $name}) RETURN p
```

Output shows each statement's result with its source label:
```
-- queries/setup.cypher          ← first statement (line 1, no line number shown)
(no rows returned)

-- queries/setup.cypher:8        ← second statement starts at line 8
╭──────┬──────╮
│ name │ age  │
╞══════╪══════╡
│ Alice│ 30   │
╰──────┴──────╯
1 row(s)

2 statement(s) executed, 1 row(s) returned.
```

**Checkpoint**: You understand that all statements in a file share the same preflight
pass — a lint error in any statement aborts before execution begins.

---

## Exit criteria

Workflow is complete when:
- `relate query` exits `0`, and
- Results are printed or captured as intended

---

## Anti-Rationalization Table

| Excuse | Rebuttal |
|--------|----------|
| "I don't need `--write`, the query doesn't look destructive." | Write classification is AST-based — it catches `MERGE`, `SET`, `DELETE` regardless of how benign they look. The flag is for explicitness, not approval. Add it. |
| "I'll skip `--param` and inline the value directly in the Cypher string." | Inlining values risks injection and bypasses type coercion. Use `--param` or the positional map — they are the safe, correct paths. |
| "The statement has no `$parameters` so I don't need to check." | Correct — Step 2 is a no-op in that case. Proceed. |
| "I'll just use `cypher-shell` instead." | `relate query` adds preflight linting, write protection, query libraries, and JSON output. Use it for file-based and scripting workflows. Use `cypher-shell` only for its interactive REPL or transaction control. |
| "The connection failed so I'll retry with different credentials." | Check `NEO4J_PASSWORD` env var or `--password` flag. `relate query` never prompts interactively. |
| "It's a `CALL` procedure so it's read-only." | `CALL` is conservatively classified as Write because many procedures mutate the graph. Add `--write` to avoid the preflight abort. |
| "I can use JSON-style quoted keys in the map literal." | Cypher map syntax requires unquoted identifier keys: `{name: "Alice"}`. Use `--params file.json` for quoted-key JSON input. |
| "I don't need semicolons between statements in a .cypher file." | The tree-sitter-cypher parser requires `;` as a statement separator. Without it, adjacent statements are parsed as an error. Always use `;`. |

---

## Quick reference

```bash
# Inline read query
relate query -e "MATCH (n:Person) RETURN n.name AS name, n.age AS age"

# Inline parameterized write
relate query -e "CREATE (n:Person {name: $name, age: $age})" \
  --param name=Alice --param age=30 --write

# Bare-name query with map literal params
relate query find_person '{name: "Alice"}'

# Bare-name write with map literal
relate query create_person '{name: "Alice", age: 30}' --write

# file/stmt addressing
relate query person/upsert '{name: "Alice"}' --write

# Describe a query library file
relate query --describe person

# Custom query library directory
relate query --cypher-dir ./queries find_person '{name: "Alice"}'

# File query with params
relate query queries/find_person.cypher --param name=Alice

# Params from JSON file
relate query queries/upsert.cypher --params data/person.json --write

# JSON output for scripting
relate query -e "MATCH (n) RETURN count(n) AS total" --json \
  | jq '.[0].rows[0].total'

# Multi-statement file (statements must be separated by ;)
relate query queries/setup.cypher --write

# Custom Neo4j connection
relate query -e "MATCH (n) RETURN n" \
  --uri bolt://prod.example.com:7687 \
  --user reader \
  --password "$NEO4J_PASSWORD"
```

## Connection flags (global)

```
--uri <URI>           Bolt URI  [default: bolt://localhost:7687]
--user <USER>         Username  [default: neo4j]
--password <PASS>     Password  [env: NEO4J_PASSWORD]
```

`--password` and `NEO4J_PASSWORD` are required for execution. Preflight (lint,
write check, param validation, `--describe`) runs without credentials.
