# CLI Contract: relate query (Milestone 3)

Extends `specs/002-query-command/contracts/query-cli.md` and
`specs/003-query-library-params/contracts/query-cli.md`. New and changed
items are marked **[NEW]** or **[CHANGED]**.

## Synopsis

```
relate query [OPTIONS] [QUERY] [PARAMS]
```

## Arguments

Unchanged from Milestone 2. `[PARAMS]` becomes mutually exclusive with the
new `--apply` flag.

## Options

| Flag | Short | Type | Default | Description |
|------|-------|------|---------|-------------|
| `--expr` | `-e` | `String` (repeatable) | — | Inline Cypher statement; mutually exclusive with `[QUERY]` |
| `--param` | `-p` | `NAME=VALUE` (repeatable) | — | Constant parameter (applied to every row when used with `--apply`); takes precedence over `[PARAMS]` and row-derived values on conflicts |
| `--params` | — | `FILE` | — | JSON file of named parameters; mutually exclusive with `[PARAMS]` |
| `--apply` | — | `FILE` | — | **[NEW]** Apply query once per row in a `.csv`, `.json`, or `.jsonl` file; mutually exclusive with `[PARAMS]` |
| `--batch` | — | `N` (integer ≥ 1) | `1000` (applied when `--apply` is set) | **[NEW]** Rows per transaction; mutually exclusive with `--atomic`; requires `--apply` |
| `--atomic` | — | flag | off | **[NEW]** Wrap all `--apply` iterations in a single transaction; mutually exclusive with `--batch`; requires `--apply` |
| `--write` | — | flag | off | Required for statements containing write clauses |
| `--describe` | — | flag | off | Print cypherdoc documentation without executing |
| `--list` | — | flag | off | List named statements in the query library |
| `--cypher-dir` | — | `PATH` | `./cypher/` | Query library directory for bare-name resolution |
| `--json` | — | flag | off | Output results as JSON array (per-row when used with `--apply`) |
| `--help` | `-h` | flag | — | Print help |

**Global options**: `--uri`, `--user`, `--password` / `NEO4J_PASSWORD`

## Mutual Exclusions

| Pair | Error | Status |
|------|-------|--------|
| `[QUERY]` and `-e` | `Error: [QUERY] and --expr (-e) are mutually exclusive` | M1 |
| `[PARAMS]` and `--params` | `Error: [PARAMS] and --params are mutually exclusive` | M2 |
| `--apply` and `[PARAMS]` | **[NEW]** `Error: --apply and [PARAMS] are mutually exclusive` | M3 |
| `--batch` and `--atomic` | **[NEW]** `Error: --batch and --atomic are mutually exclusive` | M3 |
| `--batch`/`--atomic` without `--apply` | **[NEW]** `Error: --batch/--atomic require --apply` | M3 |

## Supported `--apply` File Formats

| Extension | Structure | Row → param mapping | Streams? |
|-----------|-----------|---------------------|----------|
| `.csv` | First row = headers; each subsequent row = one execution | Header name → param name; values follow `--param` coercion rules | Yes |
| `.json` | Top-level JSON array of objects (any other top-level type is an error) | Object key → param name; JSON value types preserved | No (full file parsed) |
| `.jsonl` | One JSON object per non-empty line | Object key → param name; JSON value types preserved | Yes (line-by-line) |

Any other extension is rejected before any I/O on the file's contents.

## `--help` Text (required additions for M3)

The `--help` output MUST additionally include:
- Description of `--apply`: "Apply this query once per row in a CSV, JSON
  array, or JSONL file (`.csv`, `.json`, `.jsonl`). Mutually exclusive with
  the positional [PARAMS]."
- Description of the default `--batch 1000` and the trade-off vs. `--atomic`.
- One example of `--apply` with a CSV file.
- One example of `--apply --atomic`.

## Exit Codes

Unchanged from Milestones 1 and 2.

| Code | Meaning | Cause (M3 additions in **bold**) |
|------|---------|------|
| 0 | Success | All statements executed (or `--describe`/`--list` complete) |
| 1 | Preflight failure | Lint error, missing parameter, write-without-flag, mutual exclusion, query/statement not found, **invalid `--apply` file extension, malformed CSV header, JSON top-level not an array, empty input with required params** |
| 2 | Runtime failure | Neo4j connection error, query execution error, **batch row failure** |
| ≥3 | Internal error | I/O error, parse failure, **malformed JSONL line treated as I/O-class** |

## Output Format

### Human-readable (default)

**Single-row (M1/M2)**: unchanged.

**Batch (`--apply`)**:

```
[100/?] applied row 100
[200/?] applied row 200
...
[1000/?] applied row 1000 (batch 1 committed)
...
5000 rows applied across 5 batches.
```

Progress lines go to stderr. The final summary line goes to stdout (so it
can be captured separately from progress).

For `--atomic`:
```
[100/?] applied row 100
...
5000 rows applied in 1 atomic transaction.
```

### JSON (`--json`) with `--apply`

```json
[
  {
    "row": 0,
    "source": "./cypher/create_person.cypher",
    "is_write": true,
    "columns": [],
    "rows": [],
    "summary": { "nodes_created": 1, "properties_set": 2, ... }
  },
  {
    "row": 1,
    "source": "./cypher/create_person.cypher",
    "is_write": true,
    "columns": [],
    "rows": [],
    "summary": { "nodes_created": 1, "properties_set": 2, ... }
  },
  ...
]
```

The `row` field is omitted when `--apply` is not in use (Milestone 1/2
schema is preserved exactly).

## Error Diagnostic Format

**[NEW]** Invalid `--apply` extension:
```
Error: --apply requires a .csv, .json, or .jsonl file
  Got: data.txt
  Hint: convert your data to CSV or JSONL, or rename if the format is already correct
```

**[NEW]** Malformed CSV header (empty or duplicate column):
```
Error: invalid CSV header in people.csv
  Empty column at position 3
  Hint: every column must have a non-empty header name
```

**[NEW]** JSON top-level not an array:
```
Error: --apply JSON file must be a top-level array of objects
  Got: object at people.json
  Hint: wrap the object in [ ... ] or use a JSONL file
```

**[NEW]** Malformed JSONL line:
```
Error: invalid JSON on line 42 of people.jsonl
  Underlying: expected value at column 17
```

**[NEW]** First-row preflight failure:
```
Error: missing required parameter '$name'
  Source: ./cypher/create_person.cypher
  Hint: column 'name' is not present in people.csv (headers: id, age, home)

  --- Documentation ---
  Create a new person.
  @param {string} name      - Unique name for the person
  @param {integer} [age=0]  - Age in years
```

**[NEW]** Batch row failure (default / `--batch N`):
```
Error on row 1012: Neo.ClientError.Schema.ConstraintValidationFailed
  Source: ./cypher/create_person.cypher
  1000 rows committed (1 batch), 11 rows in current batch rolled back.
  Underlying: Node already exists with label `Person` and property `name` = 'Alice'
```

**[NEW]** Batch row failure (`--atomic`):
```
Error on row 1012: Neo.ClientError.Schema.ConstraintValidationFailed
  Source: ./cypher/create_person.cypher
  Transaction rolled back. 0 rows committed.
  Underlying: Node already exists with label `Person` and property `name` = 'Alice'
```

## Usage Examples

```sh
# Apply a query to every row of a CSV
relate query create_person --apply people.csv --write

# Same, but commit every row individually for max durability
relate query create_person --apply people.csv --batch 1 --write

# Wrap the whole apply in one transaction (small data sets only)
relate query create_person --apply people.json --atomic --write

# Inject a constant value across all rows
relate query create_person --apply people.csv \
  --param tenant=acme --write

# Machine-readable per-row results
relate query create_person --apply people.jsonl --write --json > results.json

# Override the query library directory
relate query --cypher-dir ./queries create_person --apply people.csv --write
```
