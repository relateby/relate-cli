# Implementation Plan: relate query — Batch Execution

**Branch**: `004-batch-execution` | **Date**: 2026-05-11 | **Spec**: [spec.md](spec.md)
**Input**: Feature specification from `specs/004-batch-execution/spec.md`

## Summary

Extend `relate query` with `--apply <FILE>` to run a single parameterized query
once per row of a CSV, JSON array, or JSONL data file — the client-side
equivalent of `LOAD CSV`. Add a three-tier transaction model (`--batch N`
default 1000, `--batch 1` per-row, `--atomic` all-rows-one-tx). Preflight runs
once against a synthetic parameter set built from the first row; progress
streams to stderr; `--json` extends per-row results with a `row` index. All
new behavior is additive — Milestones 1 and 2 invocations are unchanged.

## Technical Context

**Language/Version**: Rust 1.85.0 (MSRV unchanged)
**Primary Dependencies**: All of Milestones 1 and 2, plus:
  - `csv = "1.3"` (new) — streaming CSV parser with header support; the
    de-facto standard CSV crate for Rust, well-tested against quoting/escaping
    edge cases
**Storage**: N/A
**Testing**: `cargo test`; unit tests for format detection, row-to-param
  mapping, per-mode transaction boundaries; integration tests via `assert_cmd`
  using `tempfile`-staged data files; the integration suite already has
  optional Neo4j-backed tests gated by `NEO4J_PASSWORD` and reuses the
  existing Milestone 1/2 test harness
**Target Platform**: macOS, Linux, Windows (unchanged)
**Project Type**: CLI tool (single crate, no workspace; unchanged)
**Performance Goals** (informational; no spec-level SC asserts these): a
  10,000-row JSONL batch should complete in under 30 seconds with default
  `--batch 1000` against a local Neo4j (transaction overhead, not driver/wire
  overhead, should dominate); per-row progress signal latency under 100 ms.
  Drift below these targets is a smell to investigate, not a CI failure.
**Constraints**: No interactive prompts; single binary; JSONL input MUST
  stream (no full-file buffering); JSON array input parses fully (documented
  trade-off); preflight remains synchronous and connectionless

## Constitution Check

*GATE: All four principles verified. No violations.*

| Principle | Status | Notes |
|-----------|--------|-------|
| I. CLI-Friendly | ✅ Pass | `--apply`/`--batch`/`--atomic` are pure flags; progress goes to stderr so stdout stays composable; exit codes unchanged (0/1/2/≥3) |
| II. Human-Readable | ✅ Pass | Progress lines name the current row index; failure messages name the row, the Neo4j error, and the partial-commit state (committed/rolled back counts) |
| III. Agent-Friendly | ✅ Pass | `--json` output is an array of per-row results with a stable schema (`row`, `source`, `columns`, `rows`, `summary`); `skills/relate-query/SKILL.md` MUST be updated before M3 is considered complete |
| IV. Self-Contained Help | ✅ Pass | `--help` updated with `--apply`, `--batch`, `--atomic` (default and mutex), and at least one batch example |

No Complexity Tracking entries required.

## Project Structure

### Documentation (this feature)

```text
specs/004-batch-execution/
├── plan.md              # This file
├── research.md          # Phase 0 — technology decisions (Decisions 13–17)
├── data-model.md        # Phase 1 — new and modified types
├── contracts/
│   └── query-cli.md     # Phase 1 — updated CLI contract (M3 additions)
├── quickstart.md        # Phase 1 — user-facing walkthrough
├── checklists/
│   └── requirements.md  # /speckit-specify quality checklist
└── tasks.md             # Phase 2 output (/speckit-tasks — not yet created)
```

### Source Code

```text
src/
├── cli.rs               # Modify QueryArgs:
│                        #   +apply: Option<PathBuf>
│                        #   +batch: Option<usize>     (default applied in run())
│                        #   +atomic: bool
└── commands/
    └── query.rs         # Extend with new sections (all within the single file):
                         #   — data file format detection (new)
                         #   — row reader trait + CSV/JSON/JSONL impls (new)
                         #   — apply-mode dispatcher (new)
                         #   — batched execute_queue_apply (new)
                         #   — preflight integration: synthesise param set
                         #     from first row (new helper)
                         #   — JSON output: per-row JsonResult with "row" field
                         #     (extended)
```

New dependency in `Cargo.toml`:
```toml
csv = "1.3"
```

No source files are renamed; no new modules are created. All new code lives
under `src/commands/query.rs` alongside Milestone 1/2 code, matching the
existing single-file convention. (If `query.rs` exceeds a reviewable size,
splitting into a `query/` submodule is an out-of-scope follow-up.)

### Skills (required by Constitution III)

```text
skills/
└── relate-query/SKILL.md    # Update to document --apply, --batch, --atomic,
                             # supported file formats, and the per-row JSON
                             # output schema. The anti-rationalization table
                             # gains a row for "transaction mode picked
                             # silently": always confirm with the user when
                             # the data set is large and the mode is
                             # implied rather than asked for.
```

## Phase 0: Research

See [research.md](research.md) — all unknowns resolved. Summary:

- **Decision 13 — Data file format detection**: dispatch on file extension
  (`.csv`, `.json`, `.jsonl`); no content sniffing. Anything else is a hard
  error.
- **Decision 14 — Streaming vs buffered**: CSV streams via `csv::Reader`;
  JSONL streams line-by-line via `BufReader`; JSON arrays parse fully into
  `Vec<serde_json::Value>`. The trade-off is documented in `--help` and the
  skill.
- **Decision 15 — Row → ParamMap conversion**: map keys/headers become param
  names directly; values are coerced like the M2 positional map literal where
  the source is unambiguous (JSON values keep their JSON types; CSV values
  follow the `--param` flag coercion rules).
- **Decision 16 — Transaction model**: explicit `Graph::start_txn()` per
  batch in the default and `--batch N` modes; one outer `Txn` wrapping the
  entire `--apply` loop in `--atomic`. `Txn::execute` per row inside the
  active transaction.
- **Decision 17 — Preflight strategy**: read the first row eagerly (the
  reader yields it, the rest of the iterator is unaffected), build a
  synthetic `ParamMap`, and run all three preflight stages once before any
  Neo4j connection opens. Subsequent rows are not preflighted; runtime
  errors surface per-row during execution.

## Phase 1: Design & Contracts

See [data-model.md](data-model.md) for new and modified types.
See [contracts/query-cli.md](contracts/query-cli.md) for the updated CLI contract.
See [quickstart.md](quickstart.md) for a hands-on walkthrough.

### Key Design Decisions

**`RowReader` trait** (internal abstraction over data file formats):

```rust
trait RowReader {
    /// Yields the next row as a parameter map, or None at EOF.
    /// Errors (malformed CSV row, invalid JSON line, etc.) abort the run.
    fn next_row(&mut self) -> Option<Result<ParamMap>>;
}
```

Three concrete impls — `CsvRowReader`, `JsonArrayRowReader`, `JsonlRowReader`
— constructed by a `open_row_reader(path: &Path) -> Result<Box<dyn RowReader>>`
factory that dispatches on extension.

**Apply-mode dispatcher** (replaces the existing `execute_queue` call when
`--apply` is set):

```rust
async fn execute_queue_apply(
    queue: &[StatementEntry],
    constants: &ParamMap,        // values from --param flags only
    reader: Box<dyn RowReader>,
    mode: BatchMode,             // Batched(N) | Atomic
    neo4j: &Neo4jArgs,
    json: bool,                  // affects per-row stdout buffering
) -> Result<Vec<QueryResult>>;
```

**Preflight integration**: before any connection, read row 0 from the
reader, merge with `constants`, build the full effective `ParamMap`, and
run `preflight_lint`/`preflight_write`/`preflight_params` against that
synthetic map. Row 0 is then re-emitted as the first iteration's data
(the reader yields row 0 once, the apply loop re-receives it via a
"peeked" cache held by the factory wrapper).

**Transaction boundaries**:

```
Default / --batch N:
  loop over rows:
    if no active tx: tx = graph.start_txn(); rows_in_batch = 0
    tx.execute(query.bind(row_params))
    rows_in_batch += 1
    if rows_in_batch == N: tx.commit(); tx = None
  if tx is Some: tx.commit()  # final partial batch

--atomic:
  tx = graph.start_txn()
  loop over rows: tx.execute(...)
  tx.commit()
  on any error: tx.rollback() and propagate
```

On error mid-batch (default / `--batch N`): the active `Txn` is rolled back
and the run aborts with row-index + "committed N rows in prior batches,
rolled back M rows in current batch". Prior batches stay committed.

**Constants vs per-row**: `--param` flag values are merged into every row's
`ParamMap` *after* the row-derived values, so `--param` wins on key conflict.
This mirrors the M2 precedence: `[PARAMS]` map < row-derived values < `--param`
flags. The positional `[PARAMS]` map literal is rejected when `--apply` is set
(mutex), so there is no three-way merge to design around.

**Progress format** (stderr, default mode):

```
[100/?] applied row 100 (batch 1: 100/1000)
[1000/?] applied row 1000 (batch 1 committed)
[1500/?] applied row 1500 (batch 2: 500/1000)
```

For `--atomic`: `[N/?] applied row N (atomic — 1 transaction)`. The total
row count is shown as `?` because JSONL/CSV are streamed and the total is
not known up front; for JSON arrays the parsed length is known and the `?`
is replaced with the count.

**Per-row JSON output**: extends the Milestone 1 schema with a single
`"row": <0-based index>` field. The output is buffered to stdout (one JSON
array) regardless of progress lines, which go to stderr. Buffering does
not break streaming semantics on the input side — we still read row-by-row
— but the output array is held in memory until the loop completes. On
apply-loop abort (per-row failure), the partial array is flushed to stdout
before the error renders on stderr, so consumers can still parse the
rows that succeeded. This matches spec US5 scenario 2 and is acceptable
for the target scale (tens of thousands of rows).

**`--apply` with multi-statement queues**: each row runs every statement in
the queue in order, within the active transaction. Per-row error messages
identify both the row index and the statement source. With `--atomic`, all
statements across all rows share one transaction.

### Agent context update

The `<!-- SPECKIT START -->` block in `CLAUDE.md` is updated to point at
`specs/004-batch-execution/plan.md` so future agent sessions load this plan
as context.

## Re-evaluation Post-Design

Constitution Check re-evaluated after Phase 1 design — no new principles
challenged. Specifically:

- The new `csv` dependency is a single, well-known crate and does not
  introduce async or daemon-like behavior (Standard II: Single binary).
- Progress reporting to stderr does not violate any principle and explicitly
  supports principle I (composable with shell pipelines).
- The transaction model uses neo4rs's documented `Txn` API; no custom
  connection-pool logic or shared mutable state is introduced.

## Complexity Tracking

> Not applicable — all gates pass without justifications.

| Violation | Why Needed | Simpler Alternative Rejected Because |
|-----------|------------|-------------------------------------|
| — | — | — |
