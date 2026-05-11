# Data Model: relate query — Batch Execution (Milestone 3)

Extends the data models in `specs/002-query-command/data-model.md` and
`specs/003-query-library-params/data-model.md`. New types and modifications
only.

---

## New: BatchMode

The transaction model for an `--apply` run. Constructed from `QueryArgs`
after the mutex check.

```rust
enum BatchMode {
    /// Commit every N rows. N defaults to 1000.
    Batched(usize),
    /// Wrap all rows in a single transaction.
    Atomic,
}
```

`BatchMode::from_args(args: &QueryArgs) -> Result<Self>` enforces:
- `--batch` and `--atomic` are mutually exclusive.
- `--batch 0` is rejected with a clear error (must be ≥ 1).
- Absence of both flags yields `Batched(1000)`.

---

## New: RowReader trait

Internal abstraction over the three data file formats. Implementations are
not exposed to other modules.

```rust
trait RowReader {
    /// Yields the next row as a parameter map, or None at EOF.
    /// A returned Err aborts the entire run; the caller surfaces the
    /// row/line index in the error message.
    fn next_row(&mut self) -> Option<Result<ParamMap>>;

    /// Total row count if known up front (JSON array), else None.
    /// Used only to render progress as `[N/M]` vs. `[N/?]`.
    fn total_hint(&self) -> Option<usize> { None }
}
```

Three concrete impls live in `src/commands/query.rs`:

- `CsvRowReader` — wraps `csv::Reader<File>`, owns the header row.
- `JsonArrayRowReader` — wraps `std::vec::IntoIter<serde_json::Value>` (the
  full file is parsed at construction time).
- `JsonlRowReader` — wraps `std::io::Lines<BufReader<File>>`.

A factory function `open_row_reader(path: &Path) -> Result<Box<dyn RowReader>>`
dispatches on file extension.

---

## New: PeekableRowReader

A thin wrapper that caches the first row so preflight can inspect it
without consuming the iteration. The apply loop sees the cached row as
iteration 0, then delegates to the underlying reader.

```rust
struct PeekableRowReader {
    inner: Box<dyn RowReader>,
    first: Option<ParamMap>,    // Some until the first next_row() call
    total_hint: Option<usize>,
}

impl PeekableRowReader {
    fn open(path: &Path) -> Result<Self>;             // also reads row 0
    fn first_row(&self) -> Option<&ParamMap>;         // for preflight
    fn next_row(&mut self) -> Option<Result<ParamMap>>;
    fn total_hint(&self) -> Option<usize>;
}
```

`open()` performs the eager first-row read so preflight has something to
work against. Empty inputs surface as `first = None`, which the caller
turns into either silent exit 0 (no required params) or an error (required
params and zero input rows).

---

## Modified: QueryResult

Adds an optional `row` index for batch results. The field is `None` for
non-batch invocations (Milestone 1 behavior) and `Some(i)` for the i-th
row of an `--apply` run.

```rust
struct QueryResult {
    source: StatementSource,
    columns: Vec<String>,
    rows: Vec<Vec<serde_json::Value>>,
    summary: QuerySummary,
    is_write: bool,
    row: Option<usize>,       // NEW: 0-based row index when produced by --apply
}
```

`row` is propagated into `JsonResult` (below) so the per-row index appears
in `--json` output.

---

## Modified: JsonResult

Adds a `row` field that is serialized when present.

```rust
#[derive(serde::Serialize)]
struct JsonResult {
    #[serde(skip_serializing_if = "Option::is_none")]
    row: Option<usize>,       // NEW
    source: String,
    is_write: bool,
    columns: Vec<String>,
    rows: Vec<serde_json::Value>,
    summary: JsonSummary,
}
```

Using `skip_serializing_if` keeps the Milestone 1 output unchanged when
`--apply` is not in use, preserving the existing JSON schema for non-batch
consumers.

---

## Modified: QueryArgs (in `src/cli.rs`)

```rust
pub struct QueryArgs {
    // ... existing M1 + M2 fields ...

    /// Apply query once per row in a CSV, JSON array, or JSONL file.
    /// Mutually exclusive with [PARAMS].
    #[arg(long)]
    pub apply: Option<PathBuf>,                   // NEW

    /// Rows per transaction for --apply [default: 1000].
    /// Mutually exclusive with --atomic.
    #[arg(long)]
    pub batch: Option<usize>,                     // NEW

    /// Wrap all --apply iterations in a single transaction.
    /// Mutually exclusive with --batch.
    #[arg(long)]
    pub atomic: bool,                             // NEW
}
```

**Mutual exclusions** (enforced in `commands::query::run`):
| Pair | Error |
|------|-------|
| `--apply` and `[PARAMS]` | `Error: --apply and [PARAMS] are mutually exclusive` |
| `--batch` and `--atomic` | `Error: --batch and --atomic are mutually exclusive` |
| `--apply` absent and `--batch`/`--atomic` present | `Error: --batch/--atomic require --apply` |

The third row is a soft check — it's user-error to set transaction flags
without `--apply` because transaction mode is meaningless in single-row
invocations.

---

## New: BatchError (internal)

A small struct that the apply loop carries through failure paths so the
human-readable and JSON renderers can both report the same partial-commit
state.

```rust
struct BatchError {
    row_index: usize,            // 0-based; row at which execution failed
    statement_source: StatementSource,  // for multi-statement queues
    underlying: neo4rs::Error,
    rows_committed: usize,       // rows in completed prior batches
    rows_rolled_back: usize,     // rows in the current (now-rolled-back) batch
    mode: BatchMode,             // for error wording (Atomic vs Batched(N))
}
```

This is an internal type only; it is not exposed in the JSON output. The
JSON output stream simply terminates at the last successful row, and the
error is written to stderr in human form (consistent with M1/M2 behavior
for runtime errors).
