# Research: relate lint Command

**Date**: 2026-05-10  
**Phase**: 0 — Dependency verification and design decisions

---

## Decision: gram-diagnostics as shared diagnostic type

**Decision**: Add `gram-diagnostics = "0.3.10"` as a direct dependency.

**Rationale**: `gram-diagnostics` 0.3.10 exports `Diagnostic`, `Severity`, `Range`, and `Position` with `#[derive(Serialize, Deserialize)]` (serde is a hard dep). `gram-data` returns `gram_diagnostics::Diagnostic` directly. `cypher-data` returns its own `cypher_data::types::Diagnostic`, so a thin conversion (`from_cypher_diagnostic`) is required — see gram-data/tree-sitter-cypher#8 for tracking the upstream alignment.

**Field note**: `Position` fields are `line: u32` and `character: u32` (not `column`). The JSON output renames `character` → `column` in the serialization wrapper to match the CLI contract schema.

**Alternatives considered**: Duplicating the type in relate — rejected; keeping a local mirror until the upstream aligns is preferable to forking the type permanently.

---

## Decision: ariadne 0.6.0 for human-readable output

**Decision**: Add `ariadne = "0.6"` as a dependency.

**Rationale**: ariadne 0.6.0 (latest) renders diagnostics with source annotations. It works with in-memory strings via `Source::from(&str)`, so relate doesn't need to read files a second time — it keeps the source string in memory after the lint call. Minimal usage pattern:

```rust
Report::build(ReportKind::Warning, ("file.cypher", span))
    .with_message("Relationship has no upper bound")
    .with_label(Label::new(("file.cypher", span)).with_message("here"))
    .finish()
    .print(("file.cypher", Source::from(source)))
```

The `span` is a `std::ops::Range<usize>` (byte offsets). `gram-diagnostics::Range` uses `(line, character)` pairs, so a conversion to byte offset is needed for ariadne. Strategy: convert `(line, character)` → byte offset by counting newlines in the source string.

**Alternatives considered**: Simple `file:line:col: severity: message` formatting — rejected because the RFC explicitly requires ariadne for consistency with upstream tools.

---

## Decision: walkdir 2.5.0 for recursive directory traversal

**Decision**: Add `walkdir = "2"` as a dependency.

**Rationale**: `std::fs::read_dir()` is non-recursive. `walkdir` (BurntSushi, widely adopted) provides a simple iterator over a directory tree. Extension filtering is straightforward:

```rust
WalkDir::new(dir).into_iter()
    .filter_map(|e| e.ok())
    .filter(|e| matches!(
        e.path().extension().and_then(|s| s.to_str()),
        Some("cypher" | "gram" | "md" | "adoc")
    ))
```

**Alternatives considered**: `std::fs::read_dir` with manual recursion — rejected as unnecessarily verbose and error-prone for nested symlink handling.

---

## Decision: regex crate for fence extraction

**Decision**: Add `regex = "1"` as a dependency for Markdown/AsciiDoc fence extraction.

**Rationale**: Extracting code-fenced blocks from Markdown and `[source,lang]` blocks from AsciiDoc requires multiline pattern matching. The `regex` crate handles this cleanly. Markdown pattern:

```rust
// (?ms) = multiline + dot-matches-newline
Regex::new(r"(?ms)^```[ \t]*(cypher|openCypher|gram)[ \t]*\n(.*?)^```[ \t]*$")
```

AsciiDoc pattern:
```rust
Regex::new(r"(?ms)^\[source,[ \t]*(cypher|openCypher|gram)\]\n----\n(.*?)\n----")
```

For each match, the fence start line is computed by counting `\n` characters before `match.start()` in the document string.

**Alternatives considered**: Tree-sitter cypherdoc — unavailable as a public lib API; relate must own this logic. Hand-rolled state machine — more code for no benefit over a well-tested regex.

---

## Decision: --lang flag missing from LintArgs — add it

**Decision**: Add `lang: Lang` field with `default_value_t = Lang::Cypher` to `LintArgs` in `src/cli.rs`.

**Rationale**: RFC-002 specifies `--lang <LANG>` for selecting the engine when using `--expr` or stdin. The current `LintArgs` struct omits this flag. It must be added before the `--expr` path can be implemented.

**Scope**: Small addition to `cli.rs`; `Lang` enum with variants `Cypher` and `Gram` implementing `clap::ValueEnum`.

---

## Decision: Cargo.toml additions

```toml
cypher-data = "0.2.2"
gram-data = "0.3.9"
gram-diagnostics = "0.3.9"
ariadne = "0.6"
walkdir = "2"
regex = "1"
serde = { version = "1", features = ["derive"] }
serde_json = "1"
```

`serde` and `serde_json` are needed for `--json` output. `gram-diagnostics` already derives Serialize/Deserialize, so relate only needs to construct an output-layer struct (or serialize directly with field renaming).
