# Data Model: relate query — Query Library and Ergonomic Parameters (Milestone 2)

Extends `specs/002-query-command/data-model.md`. New types and modifications only.

---

## New: CypherDoc

Parsed documentation for a single Cypher statement. Extracted from a
`doc_comment` node by the `tree-sitter-cypherdoc` grammar.

```rust
struct CypherDoc {
    name: String,
    description: Option<String>,
    params: Vec<ParamDecl>,
    returns_raw: Option<String>,   // raw @returns text, for --describe display
}
```

---

## New: ParamDecl

A single `@param` declaration extracted from a cypherdoc block.

```rust
struct ParamDecl {
    name: String,
    type_: String,            // type annotation text, e.g. "string", "integer"
    required: bool,           // true = required_param; false = optional_param
    default: Option<String>,  // present iff required == false
    description: Option<String>,
}
```

`ParamDecl.name` has no `$` sigil; `relate query` maps `name` → `$name` when
cross-checking against statement `$x` references in preflight Stage 3.

---

## Modified: StatementEntry

Adds an optional `doc` field to carry parsed cypherdoc for the statement.

```rust
struct StatementEntry {
    source: StatementSource,
    text: String,
    doc: Option<CypherDoc>,  // None when no /** ... */ block precedes the statement
}
```

---

## Modified: StatementSource

Adds two new variants for library resolution.

```rust
enum StatementSource {
    Inline,
    File { path: PathBuf, line: u32, statement_idx: usize },
    // NEW — bare-name resolution: label is "cypher/name.cypher"
    LibraryFile { path: PathBuf, bare_name: String },
    // NEW — file/stmt addressing: label is "cypher/name.cypher (stmt_name)"
    LibraryStatement { path: PathBuf, stmt_name: String },
}
```

`Display` for the new variants:
- `LibraryFile { path, .. }` → `path/to/file.cypher` (same as `File` single-stmt)
- `LibraryStatement { path, stmt_name }` → `path/to/file.cypher (stmt_name)`

---

## Modified: QueryArgs (in `src/cli.rs`)

```rust
#[derive(Debug, clap::Args)]
#[command(
    long_about = "Execute a parameterized Cypher statement against Neo4j.\n\n\
                  ...",
    after_help = "Examples:\n  \
                  relate query create_person '{name: \"Alice\", age: 30}' --write\n  \
                  relate query find_person '{name: \"Alice\"}'\n  \
                  relate query person/upsert '{name: \"Alice\"}' --write\n  \
                  relate query --describe person\n  \
                  relate query -e 'MATCH (n:Person) RETURN n.name'"
)]
pub struct QueryArgs {
    /// .cypher file path, bare query name, or file/stmt address (mutually exclusive with -e)
    pub query: Option<String>,              // CHANGED: was Option<PathBuf>

    /// Cypher map literal of named parameters, e.g. '{name: "Alice", age: 30}'
    /// Merged with --param flags; --param takes precedence on conflicts.
    /// Mutually exclusive with --params.
    pub params_map: Option<String>,         // NEW: second positional

    /// Inline Cypher statement (repeatable; mutually exclusive with [QUERY])
    #[arg(short = 'e', long = "expr")]
    pub expr: Vec<String>,

    /// Named query parameter NAME=VALUE (repeatable)
    #[arg(short = 'p', long = "param")]
    pub param: Vec<String>,

    /// JSON file of named parameters (--param takes precedence on conflicts)
    #[arg(long)]
    pub params: Option<PathBuf>,

    /// Allow write operations (CREATE, MERGE, SET, DELETE, REMOVE, FOREACH)
    #[arg(long)]
    pub write: bool,

    /// Print cypherdoc documentation without executing
    #[arg(long)]
    pub describe: bool,                     // NEW

    /// Query library directory for bare-name resolution [default: ./cypher/]
    #[arg(long, default_value = "./cypher/")]
    pub cypher_dir: PathBuf,               // NEW

    /// Output results as JSON
    #[arg(long)]
    pub json: bool,
}
```

**Mutual exclusion** (enforced in `commands::query::run`):
- `query` and `expr` remain mutually exclusive (existing)
- `params_map` and `params` are mutually exclusive (new)

---

## New: QueryName

An internal enum produced by `resolve_query_source()` representing the three
resolution outcomes for a `[QUERY]` argument.

```rust
enum QueryName {
    /// Explicit path, used as-is (contains path separator or .cypher suffix)
    ExplicitPath(PathBuf),
    /// Bare name, resolved to <cypher_dir>/<name>.cypher
    BareName { name: String, resolved: PathBuf },
    /// file/stmt addressing, resolved to one named statement in a file
    StmtAddress { file: PathBuf, stmt_name: String },
}
```

`resolve_query_source(query: &str, cypher_dir: &Path) -> Result<QueryName>`
is a pure function (no I/O) that classifies the string and constructs paths.
The actual file existence check happens downstream when the file is opened.
