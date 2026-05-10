# Research: relate query — Query Library and Ergonomic Parameters (Milestone 2)

Builds on the decisions in `specs/002-query-command/research.md`. Decisions are
numbered sequentially (continuing from Decision 6).

---

## Decision 7: Cypherdoc dependency

**Decision**: Add `tree-sitter-cypherdoc = "0.2"` to `Cargo.toml` as a new
dependency. Parse cypherdoc blocks using `tree_sitter_cypherdoc::LANGUAGE`
applied to the raw text extracted from `doc_comment` nodes in the cypher AST.

**Rationale**: `tree-sitter-cypherdoc` is a separate crate from
`tree-sitter-cypher`. Despite the CLAUDE.md note ("includes cypherdoc
sub-grammar"), they are distinct crates — `tree-sitter-cypher` 0.2's
`injections.scm` references the cypherdoc language for editor injection, but
does not re-export the grammar. The `doc_comment` node kind is present in the
cypher grammar and contains the raw `/** ... */` text; that text is fed
separately to a cypherdoc parser instance.

**Node kinds in `tree-sitter-cypherdoc`**:

| Node kind | Meaning |
|-----------|---------|
| `document` | Root of a cypherdoc block |
| `name` | Statement name (first word after `/**`) |
| `description` | Optional multi-line description block |
| `description_line` | One line of description text |
| `param_tag` | `@param {type} name - desc` |
| `required_param` | Param without brackets; has `.name` field |
| `optional_param` | Param with `[name=default]`; has `.name` and `.default` fields |
| `returns_tag` | `@returns {type} - desc` |
| `type_annotation` | `{type}` around a `scalar_type` |

**Usage pattern**:
```rust
let mut cypherdoc_parser = tree_sitter::Parser::new();
cypherdoc_parser.set_language(&tree_sitter_cypherdoc::LANGUAGE.into()).unwrap();

// `raw` is the text of a doc_comment node from the cypher AST
let doc_tree = cypherdoc_parser.parse(raw, None).unwrap();
let root = doc_tree.root_node();
// root.kind() == "document"
// root.child_by_field_name("name") — not a field; iterate named children
// First named child with kind "name" is the statement name
```

**Alternatives considered**: Manual regex parsing of `/** ... */` blocks —
rejected; fragile and inconsistent with how other tree-sitter grammars are used
in the codebase.

---

## Decision 8: Positional map literal parsing

**Decision**: Parse the `[PARAMS]` positional argument by wrapping it in a
synthetic `RETURN` statement (`RETURN <input>`) and running the cypher parser,
then extracting the first `map_literal` child of the `return_clause`. Walk the
`map_literal` subtree to extract key-value pairs into a `ParamMap`.

**Rationale**: The cypher grammar already has a `map_literal` node kind with
named key-value structure. Reusing the existing cypher parser avoids a second
custom parser and handles both unquoted keys (`{name: "Alice"}`) and
JSON-style quoted keys (`{"name": "Alice"}`), since both are valid in Cypher
map expressions. The `RETURN <map>` wrapping trick gives us a fully parseable
statement.

**Node structure** (tree-sitter-cypher):

```
map_literal
  map_entry*
    identifier | string_literal  (key)
    expression                   (value)
```

**Supported value types** for automatic coercion in map literals:
- `integer_literal` → `ParamValue::Integer`
- `float_literal` → `ParamValue::Float`
- `true_literal` / `false_literal` (children of `boolean_literal`) → `ParamValue::Boolean`
- `null_literal` → `ParamValue::Json(serde_json::Value::Null)`
- `string_literal` → `ParamValue::String` (strip outer quotes)
- Nested `map_literal`, `list_literal` → `ParamValue::Json` (serialize to
  `serde_json::Value` and round-trip)

**Error handling**: If parsing fails (the string is not a valid Cypher map),
exit code 1 with message: `Error: invalid parameter map — expected a Cypher
map literal like '{name: "Alice", age: 30}'`.

**Alternatives considered**: `serde_json::from_str` — rejected; fails on
unquoted keys which are idiomatic Cypher.

---

## Decision 9: Bare-name resolution

**Decision**: A `[QUERY]` argument is treated as a bare name if and only if it
contains no OS path separator (`/` on Unix, `\` on Windows) and does not end
with `.cypher`. Bare names are resolved to `<cypher_dir>/<name>.cypher` where
`cypher_dir` is `--cypher-dir` or `./cypher/` by default.

**Rationale**: The presence of a path separator unambiguously signals an
explicit path. The absence of `.cypher` on a name with no separator signals
a bare name. This covers the common case (`find_person`) while leaving
`./find_person.cypher` and `find_person.cypher` as explicit path forms.

**Resolution order**:
1. If `[QUERY]` contains a path separator or ends with `.cypher` → use as
   an explicit path (as in Milestone 1).
2. If `[QUERY]` has the form `file/stmt` with exactly one `/` and `file` is
   a bare name → file-level resolution of `<file>.cypher` + statement-level
   selection of the statement named `<stmt>` (see Decision 10).
3. Otherwise → resolve `<cypher_dir>/<name>.cypher`.

**Error messages**:
- File not found: `Error: query 'X' not found in ./cypher/  (looked for ./cypher/X.cypher)`
- Directory not found: `Error: query directory './cypher/' does not exist`

**Alternatives considered**: Also searching by `.cypher` extension strip and
alias lookup — deferred to a future milestone (project-level config).

---

## Decision 10: Statement-level addressing (file/statement)

**Decision**: If `[QUERY]` matches `^([a-zA-Z_][a-zA-Z0-9_]*)/([a-zA-Z_][a-zA-Z0-9_]*)$`
(a bare name, a `/`, and another bare name with no path separators), treat the
first component as a file name and the second as a cypherdoc statement name.

The file is resolved as a bare name per Decision 9. All statements in the file
are parsed and their cypherdoc blocks extracted. The statement whose cypherdoc
`name` node matches the second component is selected; all others are discarded.

**Disambiguation from explicit paths**: An explicit path like
`queries/find_person.cypher` contains `.cypher` and therefore is not matched
by the pattern; it goes through normal path resolution.

**Error if statement not found**: List all addressable names in the file:
```
Error: statement 'by_age' not found in ./cypher/person.cypher
  Available statements: upsert, delete, find_by_name
  Hint: use 'relate query --describe person' to see full documentation
```

**Alternatives considered**: Positional index (`person[1]`) — less readable;
CQL-style dotted notation (`person.upsert`) — conflicts with property access
syntax in other contexts.

---

## Decision 11: QueryArgs evolution for Milestone 2

**Decision**: Evolve `QueryArgs` in `src/cli.rs` with three additions:

1. `query` field type changes from `Option<PathBuf>` to `Option<String>` so
   bare names, `file/stmt` addresses, and explicit paths can all be represented
   without PathBuf normalisation stripping the `/stmt` suffix.
2. Add `params_map: Option<String>` as the second positional argument — a
   Cypher map literal string. Mutually exclusive with `--params` (enforced in
   `run()`).
3. Add `--describe` (bool flag) — print documentation without executing.
4. Add `--cypher-dir <PATH>` (`Option<PathBuf>`) — override the query library
   directory.

**Mutual exclusion table** (all enforced in `run()`, not clap):

| Pair | Error |
|------|-------|
| `[QUERY]` + `-e` | existing |
| `[PARAMS]` + `--params` | new: `[PARAMS] and --params are mutually exclusive` |

**Alternatives considered**: Using clap `conflicts_with` for `params_map` vs
`params` — clap's handling of two optional positionals with `conflicts_with` is
unreliable; enforcing in `run()` is consistent with existing practice.

---

## Decision 12: `--describe` output format

**Decision**: Print to stdout (it is primary output, not a diagnostic). For a
single-statement source, print one documentation block. For multi-statement
files, print blocks in file order separated by `──` ruler lines.

**Format per statement**:
```
── <file/name> ────────────────────────────────────────────────────────────────
<description>

@param {type} name          - Description
@param {type} [opt=default] - Description
@returns {[col: type][]}    - Description

  <statement text, indented 2 spaces>
```

For a `.cypher` file with no cypherdoc, the output is:
```
── <source> ───────────────────────────────────────────────────────────────────
(no documentation)

  <statement text, indented 2 spaces>
```

`--describe` exits with code 0. It does not open a Bolt connection.

**Alternatives considered**: Pager output for long files — deferred; `less`
piping works out of the box on Unix.
