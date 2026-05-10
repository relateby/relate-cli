---
name: relate-query
description: >
  Execute parameterized Cypher statements against Neo4j using `relate query`.
  Use this skill when you need to run Cypher from the command line — inline or
  from a .cypher file — with preflight linting, write protection, and named
  parameter support. Covers single statements, multi-statement files, JSON output,
  and safe write operations.
triggers:
  - "relate query"
  - "run cypher"
  - "execute cypher"
  - "query neo4j"
  - "cypher query"
  - "parameterized cypher"
  - "send query to neo4j"
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
| Multi-statement `.cypher` file | `relate query queries/setup.cypher` |

`-e` and a file path are **mutually exclusive** — use one or the other.

**Checkpoint**: You know whether to use `-e` or a file path.

---

## Step 2 — Supply parameters

Cypher statements may reference `$name`-style parameters. Supply values with:

| Situation | Flag |
|-----------|------|
| Single scalar value | `--param name=Alice` |
| Multiple scalars | `--param name=Alice --param age=30` |
| Complex types (lists, maps) or many params | `--params params.json` |
| Mix (file as base, flag overrides) | `--params base.json --param name=Alice` |

`--param` always takes precedence over `--params` on key conflicts.

Type coercion for `--param`:
- Fully numeric → integer (`30` → `30`)
- Numeric with decimal → float (`3.14` → `3.14`)
- `true` / `false` → boolean
- Anything else → string

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

# File containing writes
relate query queries/upsert_person.cypher --param name=Alice --write
```

Omitting `--write` on a write statement exits with code 1 and names the offending
clause and source — before any Bolt connection is opened.

**Checkpoint**: You have added `--write` if and only if the statement mutates the graph.

---

## Step 4 — Choose output format

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

## Step 5 — Run and interpret exit code

| Exit code | Meaning | Next action |
|-----------|---------|-------------|
| `0` | All statements executed successfully | Done |
| `1` | Preflight failure | Read stderr — lint error, missing param, or write-without-flag |
| `2` | Runtime failure | Neo4j connection refused or query execution error |
| `≥3` | Internal error | I/O or parse failure; check the file path |

**Checkpoint**: Exit code is captured. If non-zero, act on the stderr diagnostic.

---

## Step 6 — Multi-statement files

A `.cypher` file may contain multiple statements. `relate query` executes them in
order, fail-fast: the first error (preflight or runtime) aborts the run.

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
| "I'll skip `--param` and inline the value directly in the Cypher string." | Inlining values risks injection and bypasses type coercion. Use `--param` — it is the safe, correct path. |
| "The statement has no `$parameters` so I don't need to check." | Correct — Step 2 is a no-op in that case. Proceed. |
| "I'll just use `cypher-shell` instead." | `relate query` adds preflight linting, write protection, and JSON output. Use it for file-based workflows and scripting. Use `cypher-shell` only when you specifically need its interactive REPL or transaction control. |
| "The connection failed so I'll retry with different credentials." | Check `NEO4J_PASSWORD` env var or `--password` flag. `relate query` never prompts interactively. |
| "It's a `CALL` procedure so it's read-only." | `CALL` is conservatively classified as Write because many procedures mutate the graph. Add `--write` to avoid the preflight abort. |

---

## Quick reference

```bash
# Inline read query
relate query -e "MATCH (n:Person) RETURN n.name AS name, n.age AS age"

# Inline parameterized write
relate query -e "CREATE (n:Person {name: $name, age: $age})" \
  --param name=Alice --param age=30 --write

# File query with params
relate query queries/find_person.cypher --param name=Alice

# Params from JSON file
relate query queries/upsert.cypher --params data/person.json --write

# JSON output for scripting
relate query -e "MATCH (n) RETURN count(n) AS total" --json \
  | jq '.[0].rows[0].total'

# Multi-statement file
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
write check, param validation) runs without credentials.
