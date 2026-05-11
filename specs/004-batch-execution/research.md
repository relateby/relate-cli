# Research: relate query — Batch Execution (Milestone 3)

Builds on the decisions in `specs/002-query-command/research.md` and
`specs/003-query-library-params/research.md`. Decisions are numbered
sequentially (continuing from Decision 12).

---

## Decision 13: Data file format detection

**Decision**: Dispatch on file extension alone. `.csv` → CSV reader, `.json`
→ JSON array reader, `.jsonl` → JSONL line reader. Any other extension is a
hard error before any I/O of the file's contents.

**Rationale**: Content sniffing for tabular formats is fragile (CSV vs.
TSV vs. headerless CSV vs. JSON-with-leading-whitespace) and adds an
implicit-magic surface that is hard to document in `--help`. Users already
manage extensions for `.cypher` files via existing M1/M2 conventions; the
batch input convention follows the same posture.

**Error format**:
```
Error: --apply requires a .csv, .json, or .jsonl file
  Got: data.txt
```

**Alternatives considered**:
- Content sniffing (read first byte: `[` → JSON array, `{` → JSONL, else
  CSV) — rejected; ambiguous for malformed input and surprising when a CSV
  file happens to start with `{`.
- A `--format <csv|json|jsonl>` override flag — deferred; can be added
  later without breaking the extension-based default.

---

## Decision 14: Streaming vs. buffered reading

**Decision**: CSV and JSONL stream row-by-row from disk via `csv::Reader`
and `BufReader::lines()` respectively. JSON arrays parse the full file into
`Vec<serde_json::Value>` up front because `serde_json` does not offer
random-position streaming over arrays in a way that is robust against
arbitrary whitespace and nested structures.

**Rationale**: JSONL is the recommended format for large datasets precisely
because it streams; JSON arrays are a convenience for users who already have
their data in that shape but become a memory hazard above some unspecified
threshold (well into the millions of rows on contemporary machines, but
exact numbers depend on row size). The threshold is documented in `--help`
and the skill rather than enforced — users who hit memory pressure should
convert to JSONL.

**Behavioural implications**:
- CSV and JSONL: total row count is unknown until EOF; progress lines show
  `[N/?]` rather than `[N/M]`.
- JSON array: total row count is known after the initial parse; progress
  lines show `[N/M]`.
- All three readers produce errors with a row/line index when individual
  records are malformed (CSV: comfy parser error; JSONL: `serde_json::from_str`
  error with the line number; JSON array: parse error before the loop begins).

**Alternatives considered**:
- `simd-json` for streaming JSON array parsing — rejected; introduces a
  large dependency and a Cargo features matrix for marginal gain at the
  current target scale.
- Requiring `--apply` to only accept JSONL — rejected; user-hostile when the
  input is already a small JSON array, and JSON arrays of a few hundred to a
  few thousand rows are common in practice.

---

## Decision 15: Row → ParamMap conversion

**Decision**: Each row produces a `ParamMap` whose keys are the column
names (CSV header) or object keys (JSON / JSONL). Value coercion depends on
the source:

| Source | Coercion rule |
|--------|---------------|
| JSON / JSONL value | Keep JSON type: integer → `ParamValue::Integer`, number with decimal → `Float`, boolean → `Boolean`, string → `String`, null → `Json(Null)`, array/object → `Json(...)` |
| CSV cell | Same rules as the `--param NAME=VALUE` flag: numeric → integer; numeric with decimal → float; `true`/`false` → boolean; anything else → string. Empty cells become `String("")`. |

**Rationale**: JSON types are unambiguous and should not be re-coerced.
CSV cells are textual and follow the same convention as `--param` flags so
that users have one mental model for type coercion across the tool.

**CSV NULL caveat**: CSV cannot natively express Cypher NULL. An empty cell
maps to `String("")`. Users who need NULL must use JSON/JSONL (where `null`
is unambiguous) or restructure the query to handle empty strings explicitly
(e.g., `WITH coalesce($home, null) AS home`). This is the same trade-off
made by `LOAD CSV`.

**Header validation** (CSV only): the first row is the header; empty header
cells or duplicate header names are an error caught before any execution.

**Alternatives considered**:
- Always interpret CSV cells as strings; require explicit casts in the
  query — rejected; mismatches the M2 `--param` flag behaviour and forces
  users to write more verbose Cypher for the common case.
- Type-aware CSV via a sidecar schema file — deferred; can be layered on
  top later without changing the default behaviour.

---

## Decision 16: Transaction model

**Decision**: Use neo4rs's explicit `Txn` API (`graph.start_txn()`,
`tx.execute(q).await`, `tx.commit().await`, `tx.rollback().await`) to
implement the three-tier transaction model.

```
Default (--batch 1000) / --batch N:
  loop:
    open Txn if none active
    tx.execute(query bound to row params)
    on per-row error: tx.rollback(); abort with committed/rolled-back counts
    if rows-in-batch == N: tx.commit(); tx = None
  at end of input: if tx is Some, tx.commit()

--atomic:
  tx = graph.start_txn()
  for each row: tx.execute(...)
  on error: tx.rollback(); abort
  at end: tx.commit()
```

**Rationale**: The `Txn` handle holds a dedicated connection from the pool,
so per-row `tx.execute` calls do not pay the cost of acquiring a new
connection per row. Auto-commit per query (`graph.execute(q)`) would defeat
the purpose of `--batch N`. Using one `Txn` per batch matches Neo4j's
recommended bulk-load pattern and gives a clean rollback boundary at each
batch.

**`--batch 1` is per-row commit**: the loop simply commits and closes the
`Txn` after each row. Throughput is lower (one round-trip commit per row)
but durability is per-row.

**Failure recovery**: there is no automatic retry. A failed batch is rolled
back and the run aborts. Re-running is the user's responsibility.

**Alternatives considered**:
- One auto-commit query per row (`graph.execute(q)` per row) — rejected;
  cannot implement batching, far slower at scale.
- Using neo4rs's `Txn::run_queries(...)` over a `Vec<Query>` batched in
  memory — rejected; less clear in the loop, and accumulating queries
  in memory partially defeats the streaming reader's purpose.

---

## Decision 17: Preflight strategy for batch runs

**Decision**: Read the first row eagerly from the data reader, merge it
with `--param` constants to form a synthetic effective `ParamMap`, and run
all three preflight stages (lint → write classification → parameter
validation) against that map. If preflight passes, the data reader replays
the cached first row to the apply loop as iteration 0. Subsequent rows are
not preflighted.

**Rationale**: Running preflight against every row would either require
opening a connection per row (defeating the purpose of preflight) or
collecting all rows up front (defeating streaming). Validating once against
row 0 catches the common errors — wrong parameter names, missing required
parameters, write-without-flag — before any Neo4j connection opens, which
is the documented contract from Milestone 1.

**Mechanism**: a thin "peekable" wrapper around `Box<dyn RowReader>` caches
the first row internally; the apply loop's iterator yields the cached row
first, then delegates to the underlying reader. No change to the
`RowReader` trait itself.

**Edge case — empty input**: if the data file has zero rows after headers
(CSV with only a header line, empty JSON array, empty JSONL file), the run
exits with code 0 after preflight against an *empty* parameter set. This
means a query with required parameters and an empty input file fails
preflight before any connection — which is the desired behavior, because
running zero queries with missing parameters is still a programming error
worth reporting.

Actually — empty input + required params is a degenerate case. The
implementation MUST distinguish:
- Empty input with no required params → exit 0 silently (no work to do).
- Empty input with required params → exit 1 with a clear "no input rows
  found in <file>" message and the cypherdoc hint, since the user almost
  certainly meant to apply to a non-empty file.

**Alternatives considered**:
- Sample N rows and union their keys for preflight — rejected; unnecessary
  complexity, and row 0 is representative under the documented assumption
  that all rows share a shape.
- No preflight at all under `--apply` (rely on per-row runtime errors) —
  rejected; users would only discover missing parameters after a connection
  is opened, which contradicts the M1 "fail fast before connecting" contract.

---

## Cross-cutting note: skill update is required

Constitution III mandates that `skills/relate-query/SKILL.md` is updated
before the milestone is considered complete. The skill MUST include:

- `--apply` workflow steps (file format detection, transaction-mode choice,
  preflight, progress monitoring).
- An anti-rationalization row for "silently picking `--batch 1000` when the
  user did not specify a mode and the data set is large" — the skill should
  prompt the user to confirm the transaction mode for batches above some
  threshold (initial proposal: 10,000 rows).
- An exit-criteria item for "the user has seen at least one progress
  line and any error message includes the row index and partial-commit
  state".
