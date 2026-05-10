# Research: relate query — Single Query Execution

## Decision 1: neo4rs 0.9.x query execution API

**Decision**: Use `Graph::execute(query(...).param(...))` for result-returning queries.
Use `Graph::run(...)` for fire-and-forget write queries. Consume results with
`RowStream::next().await`.

**Rationale**: `execute` returns a `RowStream`; `run` discards rows. For `relate query`
all statements use `execute` so we can uniformly capture result metadata (rows returned,
counters for writes).

**Bolt parameter binding**:
```rust
let q = query("MATCH (n:Person {name: $name}) RETURN n.name AS name")
    .param("name", "Alice");
let mut stream = graph.execute(q).await?;
while let Ok(Some(row)) = stream.next().await {
    let name: String = row.get("name")?;
}
```

**Parameter type mapping** from `--param NAME=VALUE` string to `BoltType`:
- Fully numeric integer (e.g. `30`) → `i64` → `BoltInteger`
- Numeric with decimal (e.g. `3.14`) → `f64` → `BoltFloat`
- `true` / `false` (exact, case-sensitive) → `BoltBoolean`
- Anything else → `String` → `BoltString`

Complex types (lists, maps, null) require `--params FILE` JSON, which is deserialized
via `serde_json::Value` → `BoltType` conversion (neo4rs provides this via `Into<BoltType>`
for `serde_json::Value`).

**Alternatives considered**: Using neo4rs `transaction()` API — deferred to Milestone 3
(`--apply --atomic`). For Milestone 1, auto-commit per statement suffices.

---

## Decision 2: Multi-statement file splitting

**Decision**: Parse the full file with `tree_sitter_cypher::language()` and iterate
`statement` child nodes of the root. Extract each statement's source text via its
byte range in the original file content. Track the 0-based start line for the source
label (`file.cypher:<line+1>`).

**Rationale**: tree-sitter-cypher's root node is `cypher_statements` with zero or more
`cypher_statement` children. Using the parser gives us consistent statement boundaries
identical to what lint uses.

**Statement node path** (verify against tree-sitter-cypher grammar during implementation):
```
root → statement* where statement has kind "statement"
```

**Alternatives considered**: Splitting on `;` with regex — rejected because semicolons
can appear in string literals and in subqueries.

---

## Decision 3: Write clause classification

**Decision**: Walk the statement's AST subtree. If any node's `kind()` matches the
write clause set, classify the statement as Write. Otherwise Read.

**Write clause node kinds** (verify names against tree-sitter-cypher grammar during
implementation — these are the expected names based on the Cypher grammar):

| Cypher clause | Expected tree-sitter node kind |
|--------------|-------------------------------|
| CREATE | `create_clause` |
| MERGE | `merge_clause` |
| SET | `set_clause` |
| DELETE / DETACH DELETE | `delete_clause` |
| REMOVE | `remove_clause` |
| FOREACH | `foreach_clause` |

**CALL classification**: Any `call_clause` is conservatively classified as Write
unless its procedure name is in a known-safe read-only allowlist (e.g. `db.labels`,
`db.relationshipTypes`, `apoc.meta.*`). This is a conservative default; see RFC-003
Unresolved Questions.

**Rationale**: AST-based classification is accurate and reuses the same parser
instance we already have for splitting.

**Alternatives considered**: Regex on text — rejected; too fragile for nested queries
and WITH chains.

---

## Decision 4: Table output crate

**Decision**: Add `comfy-table = "7"` to `Cargo.toml`. Use `Table::new()` with
`ContentArrangement::Dynamic` and `UTF8_FULL` preset for the default Unicode table
output.

**Rationale**: `comfy-table` is the most widely used Rust table rendering crate,
supports Unicode box-drawing, is actively maintained, and has simple ergonomics for
dynamic column sets (which we need since Cypher result column names vary per query).

**Usage pattern**:
```rust
use comfy_table::{Table, presets::UTF8_FULL, ContentArrangement};
let mut table = Table::new();
table.load_preset(UTF8_FULL)
     .set_content_arrangement(ContentArrangement::Dynamic)
     .set_header(columns)
     .add_rows(rows);
println!("{table}");
```

**Alternatives considered**: `tabled` — also good; slightly more complex API for
dynamic use. `prettytable-rs` — older, less maintained.

---

## Decision 5: Async vs sync

**Decision**: `commands::query::run` is `async fn run(...) -> Result<()>`, consistent
with other Neo4j commands (`write`, `read`, `mcp`). The preflight pipeline (lint,
classification, param validation) runs synchronously within the async function.

**Rationale**: neo4rs requires async for all Bolt I/O. Preflight is synchronous
tree-sitter work but is a tiny fraction of overall latency.

**Alternatives considered**: Spawning preflight on a blocking thread — unnecessary
complexity; preflight takes < 50ms for typical query files.

---

## Decision 6: Exit code implementation

`std::process::exit(N)` is used for codes 1 (preflight failure) and 2 (runtime
failure), matching the pattern established in `commands::lint::run`. Code 0 is the
implicit return from `Ok(())`. Code 3 (internal error) propagates as `Err` from
`main`, which prints the error and exits non-zero via `anyhow`.

**Implementation note**: `main.rs` must handle the `Query` arm with the same pattern
as `Lint` — calling `std::process::exit` for known failure codes and propagating
`Err` for internal failures.
