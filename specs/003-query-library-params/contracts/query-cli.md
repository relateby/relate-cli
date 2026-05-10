# CLI Contract: relate query (Milestone 2)

Extends `specs/002-query-command/contracts/query-cli.md`. New and changed items
are marked **[NEW]** or **[CHANGED]**.

## Synopsis

```
relate query [OPTIONS] [QUERY] [PARAMS]
```

## Arguments

| Argument | Type | Required | Description |
|----------|------|----------|-------------|
| `[QUERY]` | string **[CHANGED]** | No | `.cypher` file path, bare query name, or `file/stmt` address; mutually exclusive with `-e` |
| `[PARAMS]` | string **[NEW]** | No | Cypher map literal `{key: value, ...}`; mutually exclusive with `--params` |

### `[QUERY]` resolution rules

| Form | Resolution |
|------|-----------|
| Contains `/` or `\` (path separator) | Explicit path; used as-is |
| Ends with `.cypher` | Explicit path; used as-is |
| `bare_name` (letters, digits, `_`, no separator) | Resolved to `<cypher-dir>/<bare_name>.cypher` |
| `file/stmt` (bare name + `/` + bare name) | `<cypher-dir>/<file>.cypher`, statement named `<stmt>` |

### `[PARAMS]` syntax

A Cypher map literal: `{ key: value, ... }` or `{ "key": value, ... }`.

Value type coercion:
- Integer literals → integer
- Float literals → float  
- `true` / `false` → boolean
- `null` → null
- String literals → string (quotes stripped)
- Nested maps / lists → passed as-is (complex types)

## Options

| Flag | Short | Type | Default | Description |
|------|-------|------|---------|-------------|
| `--expr` | `-e` | `String` (repeatable) | — | Inline Cypher statement; mutually exclusive with `[QUERY]` |
| `--param` | `-p` | `NAME=VALUE` (repeatable) | — | Named query parameter; takes precedence over `[PARAMS]` on key conflicts |
| `--params` | — | `FILE` | — | JSON file of named parameters; mutually exclusive with `[PARAMS]` |
| `--write` | — | flag | off | Required for statements containing write clauses |
| `--describe` | — | flag | off | **[NEW]** Print cypherdoc documentation without executing |
| `--cypher-dir` | — | `PATH` | `./cypher/` | **[NEW]** Query library directory for bare-name resolution |
| `--json` | — | flag | off | Output results as JSON array |
| `--help` | `-h` | flag | — | Print help |

**Global options**: `--uri`, `--user`, `--password` / `NEO4J_PASSWORD`

## Mutual Exclusions

| Pair | Error |
|------|-------|
| `[QUERY]` and `-e` | `Error: [QUERY] and --expr (-e) are mutually exclusive` |
| `[PARAMS]` and `--params` | **[NEW]** `Error: [PARAMS] and --params are mutually exclusive` |

## `--help` Text (required additions for M2)

The `--help` output MUST additionally include:
- Description of bare-name resolution: "A bare query name (no path separator, no .cypher
  extension) is resolved against the query library directory (default: ./cypher/)."
- Description of `file/stmt` addressing: "Use file/stmt to target a named statement
  within a multi-statement file."
- One example of bare-name invocation
- One example of `--describe` usage

## Exit Codes

Same as Milestone 1. No new codes.

| Code | Meaning | Cause |
|------|---------|-------|
| 0 | Success | All statements executed (or `--describe` complete) |
| 1 | Preflight failure | Lint error, missing parameter, write-without-flag, mutual exclusion, query/statement not found |
| 2 | Runtime failure | Neo4j connection error, query execution error |
| ≥3 | Internal error | I/O error, parse failure |

## `--describe` Output (human-readable to stdout)

```
── <source label> ─────────────────────────────────────────────────────────────
<description>

@param {type} name          - Description
@param {type} [opt=default] - Description
@returns {[col: type][]}    - Description

  <statement text, indented 2 spaces>
```

For a statement with no cypherdoc:
```
── <source label> ─────────────────────────────────────────────────────────────
(no documentation)

  <statement text, indented 2 spaces>
```

`--describe` does **not** open a Bolt connection and does **not** run preflight
stages 2 or 3. Only parsing (implicit stage 0) occurs.

## Error Diagnostic Format

**[NEW]** Query not found:
```
Error: query 'find_person' not found in ./cypher/
  Looked for: ./cypher/find_person.cypher
  Hint: check the query library directory with --cypher-dir
```

**[NEW]** Statement not found:
```
Error: statement 'by_age' not found in ./cypher/person.cypher
  Available statements: upsert, delete, find_by_name
  Hint: use 'relate query --describe person' to see full documentation
```

**[NEW]** Invalid `[PARAMS]` map:
```
Error: invalid parameter map — expected a Cypher map literal like '{name: "Alice", age: 30}'
  Got: 'name=Alice'
  Hint: use --param name=Alice for key=value syntax
```

**[NEW]** Missing parameter with cypherdoc:
```
Error: missing required parameter '$name'
  Source: ./cypher/person.cypher (upsert)
  Hint: pass --param name=<value>

  --- Documentation ---
  Create or update a person node.
  @param {string} name      - Unique name for the person
  @param {string} [home=""] - Home city or region
```

## Usage Examples

```sh
# Bare-name query with inline map parameters
relate query find_person '{name: "Alice"}'

# Bare-name write query
relate query create_person '{name: "Alice", age: 30}' --write

# Target a named statement in a multi-statement file
relate query person/upsert '{name: "Alice"}' --write

# Describe all statements in a file
relate query --describe person

# Describe a single statement
relate query --describe person/upsert

# Override query library directory
relate query --cypher-dir ./queries find_person '{name: "Alice"}'

# Mix positional map with --param override
relate query create_person '{name: "Alice", age: 30}' --param age=31 --write
```
