# Data Model: relate query — Single Query Execution

All types are internal to `src/commands/query.rs` unless noted.

---

## StatementEntry

A single Cypher statement ready for preflight and execution.

```rust
struct StatementEntry {
    source: StatementSource,
    text: String,
}
```

Derived from `-e` inline input or parsed from a `.cypher` file.

---

## StatementSource

Tracks the origin of a statement for diagnostics and output headers.

```rust
enum StatementSource {
    Inline,                                     // -e "..."
    File { path: PathBuf, line: u32 },          // file.cypher or file.cypher:12
}
```

`Display` implementation:
- `Inline` → `<inline>`
- `File { path, line: 1 }` (single-statement or first statement) → `path/to/file.cypher`
- `File { path, line: N }` (subsequent statements) → `path/to/file.cypher:N`

---

## Classification

Read/write classification result for a statement.

```rust
enum Classification {
    Read,
    Write { first_write_kind: String },    // node kind of the first write clause found
}
```

---

## ParamValue

A typed Cypher parameter value, parsed from `--param NAME=VALUE` strings or from
a `--params` JSON file.

```rust
enum ParamValue {
    Integer(i64),
    Float(f64),
    Boolean(bool),
    String(String),
    Json(serde_json::Value),    // from --params file only; lists, maps, null
}
```

Conversion to `neo4rs::BoltType` is via `Into<BoltType>` implementations.

---

## ParamMap

The merged set of named parameters for a query execution.

```rust
type ParamMap = HashMap<String, ParamValue>;
```

Built by merging `--params` file first, then `--param` flags (which take precedence
on key collision).

---

## QueryResult

The result of executing one statement.

```rust
struct QueryResult {
    source: StatementSource,
    columns: Vec<String>,
    rows: Vec<Vec<serde_json::Value>>,
    summary: QuerySummary,
}

struct QuerySummary {
    nodes_created: u64,
    nodes_deleted: u64,
    relationships_created: u64,
    relationships_deleted: u64,
    properties_set: u64,
    labels_added: u64,
}
```

---

## QueryArgs (in `src/cli.rs`)

The clap argument struct for `relate query`, added to `src/cli.rs` and registered
as `Commands::Query(QueryArgs)`.

```rust
#[derive(Debug, clap::Args)]
pub struct QueryArgs {
    /// .cypher file path or bare query name (mutually exclusive with -e)
    pub query: Option<PathBuf>,

    /// Inline Cypher statement (repeatable; mutually exclusive with [QUERY])
    #[arg(short = 'e', long = "expr")]
    pub expr: Vec<String>,

    /// Named parameter NAME=VALUE (repeatable)
    #[arg(short = 'p', long = "param")]
    pub param: Vec<String>,

    /// JSON file of named parameters
    #[arg(long)]
    pub params: Option<PathBuf>,

    /// Allow write operations (CREATE, MERGE, SET, DELETE, etc.)
    #[arg(long)]
    pub write: bool,

    /// Output results as JSON
    #[arg(long)]
    pub json: bool,
}
```

**Mutual exclusion** (`query` vs `expr`) is enforced in `commands::query::run` rather
than via clap `conflicts_with`, since clap's handling of `Option` vs `Vec` makes
declarative mutual exclusion awkward here.
