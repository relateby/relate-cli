# Quickstart: relate query --apply (Milestone 3)

A five-minute walkthrough of batch execution. Assumes a running Neo4j with
`NEO4J_PASSWORD` exported, and the Milestone 2 query library convention
(`./cypher/<name>.cypher`).

## 1. Set up a tiny query library

```sh
mkdir -p cypher
cat > cypher/create_person.cypher <<'EOF'
/**
 * create_person
 *
 * Insert or upsert a person node.
 *
 * @param {string} name - Unique person name
 * @param {integer} [age=0] - Optional age
 * @param {string} [home=""] - Optional home city
 */
MERGE (p:Person {name: $name})
SET p.age = $age, p.home = $home
EOF
```

Sanity-check the cypherdoc with `--describe`:

```sh
relate query --describe create_person
```

## 2. Prepare a data file (CSV)

```sh
cat > people.csv <<'EOF'
name,age,home
Alice,30,Cambridge
Bob,25,London
Carol,42,Edinburgh
EOF
```

## 3. Apply the query to every row

```sh
relate query create_person --apply people.csv --write
```

What happens, in order:

1. **Format detection** — `.csv` extension picks the CSV reader.
2. **First-row preflight** — `relate query` reads `Alice,30,Cambridge` and
   builds a synthetic parameter set `{name: "Alice", age: 30, home:
   "Cambridge"}`. The preflight pipeline (lint → write-classification →
   parameter validation) runs against that map before any Neo4j connection
   opens. Missing parameters or write-without-`--write` would fail here.
3. **Connection and execution** — a Bolt connection opens; `start_txn()`
   begins the first batch. Each row's parameter map is bound to the query
   and `tx.execute(...)` runs it.
4. **Commit** — at 1000 rows (default) the transaction commits and a new
   one starts. With only 3 rows here, the final partial batch commits at
   EOF.
5. **Summary** — `3 rows applied across 1 batch.` to stdout.

Progress lines stream to stderr; redirect them with `2> progress.log` if
you want to capture them separately.

## 4. Try the other formats

JSON array:

```sh
cat > people.json <<'EOF'
[
  { "name": "Alice", "age": 30, "home": "Cambridge" },
  { "name": "Bob", "age": 25, "home": "London" },
  { "name": "Carol", "age": 42, "home": "Edinburgh" }
]
EOF

relate query create_person --apply people.json --write
```

JSONL (recommended for large datasets):

```sh
cat > people.jsonl <<'EOF'
{"name": "Alice", "age": 30, "home": "Cambridge"}
{"name": "Bob", "age": 25, "home": "London"}
{"name": "Carol", "age": 42, "home": "Edinburgh"}
EOF

relate query create_person --apply people.jsonl --write
```

## 5. Pick a transaction mode

Per-row commit (maximum durability, slowest):

```sh
relate query create_person --apply people.csv --batch 1 --write
```

Single transaction (fastest, all-or-nothing):

```sh
relate query create_person --apply people.json --atomic --write
```

## 6. Inject a constant across all rows

The `--param` flag wins on key conflicts, so you can override a column
or add a value the data file doesn't carry:

```sh
relate query create_person --apply people.csv \
  --param tenant=acme --write
```

Every row's parameter map is `{ name, age, home, tenant: "acme" }`.

## 7. Capture machine-readable per-row results

```sh
relate query create_person --apply people.csv --write --json > results.json
```

`results.json` is a JSON array with one element per row, each carrying a
`"row"` index field on top of the existing M1 result schema:

```json
[
  { "row": 0, "source": "./cypher/create_person.cypher", "is_write": true,
    "columns": [], "rows": [], "summary": { ... } },
  { "row": 1, ... },
  { "row": 2, ... }
]
```

## 8. What happens when a row fails

Introduce a row that violates a constraint (for example, a duplicate name
when the database has a uniqueness constraint on `Person(name)`):

```sh
cat > bad-people.csv <<'EOF'
name,age,home
Alice,30,Cambridge
Alice,31,Cambridge
EOF

relate query create_person --apply bad-people.csv --write
```

Output on stderr:

```
[1/?] applied row 1
Error on row 2: Neo.ClientError.Schema.ConstraintValidationFailed
  Source: ./cypher/create_person.cypher
  1 row committed (0 batches), 1 row in current batch rolled back.
  Underlying: ...
```

The same run with `--atomic` would roll back row 1 too:

```
Error on row 2: Neo.ClientError.Schema.ConstraintValidationFailed
  Source: ./cypher/create_person.cypher
  Transaction rolled back. 0 rows committed.
```

## 9. Cheat-sheet of mutex rules

- `--apply` and a positional `[PARAMS]` map literal cannot both be set.
- `--batch` and `--atomic` cannot both be set.
- `--batch`/`--atomic` without `--apply` is an error.
- `--param` flags are always allowed and apply to every row.

## 10. Next steps

- For very large datasets, prefer JSONL over JSON arrays (streaming vs
  full-file parse).
- Consider tuning `--batch` upward (e.g., `--batch 5000`) for small rows
  and high-throughput loaders, or downward (`--batch 100`) when you want
  finer-grained recovery points.
- The Milestone 2 `--describe` and `--list` flags still work — use them
  to confirm a query's required parameters before pointing `--apply` at
  a large file.
