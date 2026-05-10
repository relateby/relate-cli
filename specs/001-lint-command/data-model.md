# Data Model: relate lint Command

**Date**: 2026-05-10

## Types

### External: gram_diagnostics::Diagnostic

Provided by the `gram-diagnostics` crate. Both `cypher-data` and `gram-data` return
values of this type directly.

```rust
pub struct Diagnostic {
    pub severity: Severity,
    pub rule: String,
    pub message: String,
    pub code: Option<String>,
    pub range: Range,
}

pub struct Range {
    pub start: Position,
    pub end: Position,
}

pub struct Position {
    pub line: u32,       // 0-based
    pub character: u32,  // 0-based UTF-8 code unit offset
}

pub enum Severity { Error, Warning, Information, Hint }
```

### Internal: LintDiagnostic (src/commands/lint.rs)

Wraps a `gram_diagnostics::Diagnostic` with source context for output.

```rust
pub struct LintDiagnostic {
    pub lang: Lang,
    pub source_file: Option<PathBuf>,  // None for --expr / stdin
    pub inner: gram_diagnostics::Diagnostic,
}
```

### Internal: Snippet (src/commands/lint.rs)

Represents a code-fenced block extracted from a documentation file.

```rust
pub struct Snippet {
    pub lang: Lang,
    pub source: String,
    pub fence_start_line: u32,  // 0-based line of the opening fence in the parent document
}
```

### Output: JsonDiagnostic (src/commands/lint.rs)

Serialization-only struct for `--json` output. Field names match the RFC schema
(uses `column` rather than `gram-diagnostics`'s `character`).

```rust
#[derive(serde::Serialize)]
pub struct JsonDiagnostic<'a> {
    pub severity: &'a str,       // "error" | "warning" | "information" | "hint"
    pub rule: &'a str,
    pub message: &'a str,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub code: Option<&'a str>,
    pub file: Option<&'a str>,   // None for --expr / stdin
    pub range: JsonRange,
}

#[derive(serde::Serialize)]
pub struct JsonRange {
    pub start: JsonPosition,
    pub end: JsonPosition,
}

#[derive(serde::Serialize)]
pub struct JsonPosition {
    pub line: u32,
    pub column: u32,  // renamed from gram-diagnostics's `character`
}
```

### Enum: Lang (src/commands/lint.rs)

```rust
#[derive(Debug, Clone, Copy, clap::ValueEnum)]
pub enum Lang {
    Cypher,
    Gram,
}
```

Also added to `LintArgs` in `src/cli.rs`:

```rust
#[arg(long, value_enum, default_value_t = Lang::Cypher)]
pub lang: Lang,
```

## State Transitions

### File dispatch flow

```
Input (file path)
  └── extension?
        ├── .cypher  → cypher_data::lint::lint_file(path, opts) → Vec<gram_diagnostics::Diagnostic>
        ├── .gram    → gram_data::lint::lint_file(path, opts)   → Vec<gram_diagnostics::Diagnostic>
        ├── .md/.adoc → extract_snippets(source) → Vec<Snippet>
        │               └── for each snippet → lint_source(snippet.source, opts)
        │                     → offset diagnostics by snippet.fence_start_line
        └── other   → skip (directory walk) OR error (explicit argument)
```

### Diagnostic line offset (snippets)

```
diagnostic.range.start.line += snippet.fence_start_line
diagnostic.range.end.line   += snippet.fence_start_line
```

Applied before adding to the output list; the `inner` field is updated in place
(or a new `Diagnostic` is constructed — either is fine since `gram-diagnostics`
types derive `Clone`).
