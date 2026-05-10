# CLI Contract: relate query (Milestone 1)

## Synopsis

```
relate query [OPTIONS] [QUERY]
```

## Arguments

| Argument | Type | Required | Description |
|----------|------|----------|-------------|
| `[QUERY]` | path | No (use `-e` instead) | `.cypher` file to execute; mutually exclusive with `-e` |

## Options

| Flag | Short | Type | Default | Description |
|------|-------|------|---------|-------------|
| `--expr` | `-e` | `String` (repeatable) | — | Inline Cypher statement; mutually exclusive with `[QUERY]` |
| `--param` | `-p` | `NAME=VALUE` (repeatable) | — | Named query parameter |
| `--params` | — | `FILE` | — | JSON file of named parameters; `--param` takes precedence on conflicts |
| `--write` | — | flag | off | Required for statements containing write clauses |
| `--json` | — | flag | off | Output results as JSON array |
| `--help` | `-h` | flag | — | Print help |

**Global options** (from root `Cli`): `--uri`, `--user`, `--password` / `NEO4J_PASSWORD`

## Mutual Exclusions

- `[QUERY]` and `-e` are mutually exclusive; providing both is an error (exit code 1).

## `--help` Text (required content)

The `--help` output MUST include:
- All flags with their defaults
- At least one inline and one file usage example
- The sentence: "Statements are linted before execution. Lint is syntactic — runtime
  errors (unknown labels, constraint violations) can still occur after lint passes."
- The sentence: "Write operations (CREATE, MERGE, SET, DELETE, REMOVE, FOREACH) require
  --write."

## Exit Codes

| Code | Meaning | Cause |
|------|---------|-------|
| 0 | Success | All statements executed without error |
| 1 | Preflight failure | Lint error, missing required parameter, write-without-flag, mutual exclusion violation |
| 2 | Runtime failure | Neo4j connection error, query execution error |
| ≥3 | Internal error | I/O error, parse failure, propagated via `anyhow` |

## Human-Readable Output (default)

Per statement:
```
-- <source label>
╭──────────┬─────╮
│ name     │ age │
├──────────┼─────┤
│ Alice    │ 30  │
╰──────────┴─────╯
1 row
```

For statements with no result rows:
```
-- <source label>
(no rows returned)
```

For write statements (no RETURN):
```
-- <source label>
Created 1 node, set 2 properties.
```

Final summary line:
```
N statements executed, M rows returned.
```

All diagnostic/error output goes to **stderr**. Result tables go to **stdout**.

## JSON Output (`--json`)

A JSON array of result objects, one per statement, in execution order:

```json
[
  {
    "source": "<inline>",
    "columns": ["name", "age"],
    "rows": [
      {"name": "Alice", "age": 30}
    ],
    "summary": {
      "nodes_created": 0,
      "nodes_deleted": 0,
      "relationships_created": 0,
      "relationships_deleted": 0,
      "properties_set": 0,
      "labels_added": 0
    }
  }
]
```

`summary` is always present. `columns` and `rows` are empty arrays when the
statement returns no results.

## Error Diagnostic Format

Preflight errors are written to **stderr** in this format:

```
Error: <reason>
  Statement: <statement text or first line>
  Source: <source label>
  <optional hint>
```

Example (write-without-flag):
```
Error: write operation requires --write flag
  Statement: MERGE (n:Person {name: $name})
  Source: queries/upsert.cypher:5

  Re-run with --write to allow mutations.
```

Example (missing parameter):
```
Error: missing required parameter '$name'
  Source: queries/upsert.cypher:5
  Hint: pass --param name=<value>
```

## Usage Examples

```sh
# Inline read query
relate query -e "MATCH (n:Person) RETURN n.name AS name, n.age AS age"

# File query
relate query queries/find_person.cypher --param name=Alice

# Inline write query (requires --write)
relate query -e "CREATE (n:Person {name: $name})" --param name=Alice --write

# Multiple params from JSON file
relate query queries/find_person.cypher --params params.json

# JSON output for scripting
relate query -e "MATCH (n) RETURN count(n) AS total" --json
```
