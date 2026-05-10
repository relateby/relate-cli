# Implementation Plan: relate query — Query Library and Ergonomic Parameters

**Branch**: `003-query-library-params` | **Date**: 2026-05-10 | **Spec**: [spec.md](spec.md)
**Input**: Feature specification from `specs/003-query-library-params/spec.md`

## Summary

Extend `relate query` with a query library convention: a `./cypher/` directory
of named `.cypher` files callable by bare name (`relate query find_person`) or
by `file/stmt` address (`relate query person/upsert`). Add a positional Cypher
map literal for ergonomic multi-param invocation. Parse cypherdoc (`/** ... */`)
blocks from `.cypher` files to drive `--describe` output and improve preflight
Stage 3 error messages. All new behaviour is additive; Milestone 1 usage is
unchanged.

## Technical Context

**Language/Version**: Rust 1.85.0 (MSRV unchanged)  
**Primary Dependencies**: All of Milestone 1, plus:
  - `tree-sitter-cypherdoc = "0.2"` (new) — separate crate from tree-sitter-cypher;
    needed to parse `/** ... */` doc blocks from `.cypher` files  
**Storage**: N/A  
**Testing**: `cargo test`; unit tests for cypherdoc parsing, bare-name
  resolution, map literal parsing; integration tests via `assert_cmd`  
**Target Platform**: macOS, Linux, Windows (unchanged)  
**Project Type**: CLI tool (single crate, no workspace; unchanged)  
**Performance Goals**: `--describe` completes in < 1 second for any `.cypher`
  file up to 500 lines; no Neo4j connection required for `--describe`  
**Constraints**: No interactive prompts; single binary; no new async
  dependencies introduced; preflight and library resolution are synchronous

## Constitution Check

*GATE: All four principles verified. No violations.*

| Principle | Status | Notes |
|-----------|--------|-------|
| I. CLI-Friendly | ✅ Pass | `--describe` output to stdout; errors to stderr; all new config via flags (`--cypher-dir`); no interactive prompts |
| II. Human-Readable | ✅ Pass | `--describe` uses a ruled, indented format; parameter-missing errors include the cypherdoc block as a usage hint; map parse errors include a corrective hint |
| III. Agent-Friendly | ✅ Pass | `--json` flag unchanged and stable; `--describe` output is human-only (no `--json describe` needed for M2); `SKILL.md` must be updated before M2 is complete |
| IV. Self-Contained Help | ✅ Pass | `--help` updated with bare-name, `file/stmt`, and `--describe` examples; all new flags documented with defaults |

## Project Structure

### Documentation (this feature)

```text
specs/003-query-library-params/
├── plan.md              # This file
├── research.md          # Phase 0 — technology decisions
├── data-model.md        # Phase 1 — new and modified types
├── contracts/
│   └── query-cli.md     # Phase 1 — updated CLI contract (M2 additions)
└── tasks.md             # Phase 2 output (/speckit-tasks — not yet created)
```

### Source Code

```text
src/
├── cli.rs               # Modify QueryArgs: query String, +params_map, +describe, +cypher_dir
└── commands/
    └── query.rs         # Extend with new sections (all within the single file):
                         #   — cypherdoc parsing (new)
                         #   — query name resolution (new)
                         #   — positional map literal parsing (new)
                         #   — --describe output (new)
                         #   — extended StatementSource, StatementEntry (modified)
                         #   — Stage 3 cypherdoc integration (modified)
```

New dependency in `Cargo.toml`:
```toml
tree-sitter-cypherdoc = "0.2"
```

Note: CLAUDE.md incorrectly states that `tree-sitter-cypher 0.2` "includes
cypherdoc sub-grammar". They are separate crates on crates.io. The
`injections.scm` file in tree-sitter-cypher references the cypherdoc language
for editor injection only; the Rust bindings do not re-export it.

### Skills (required by Constitution III)

```text
skills/
└── relate-query/SKILL.md    # Update to document M2 bare-name, map literal,
                             # --describe, and --cypher-dir capabilities
```

## Phase 0: Research

See [research.md](research.md) — all unknowns resolved. Summary:

- **Cypherdoc dependency** (Decision 7): Add `tree-sitter-cypherdoc = "0.2"`;
  parse `doc_comment` node text from the cypher AST using the cypherdoc grammar.
- **Positional map literal** (Decision 8): Wrap input in `RETURN <input>` and
  parse with tree-sitter-cypher; extract the `map_literal` subtree into a
  `ParamMap`.
- **Bare-name resolution** (Decision 9): No separator + no `.cypher` suffix →
  bare name; resolved to `<cypher_dir>/<name>.cypher`.
- **Statement addressing** (Decision 10): `^bare/bare$` pattern → `file/stmt`
  address; cypherdoc `name` node used for matching.
- **QueryArgs evolution** (Decision 11): `query` changes to `Option<String>`;
  add `params_map`, `describe`, `cypher_dir`.
- **`--describe` format** (Decision 12): Ruled blocks to stdout; no Bolt
  connection; exits 0.

## Phase 1: Design & Contracts

See [data-model.md](data-model.md) for new and modified types.
See [contracts/query-cli.md](contracts/query-cli.md) for the updated CLI contract.

### Key Design Decisions

**Query name resolution** (pure function, no I/O):
```
resolve_query_source(query: &str, cypher_dir: &Path) -> QueryName
  if contains separator or ends .cypher → ExplicitPath
  if matches ^bare/bare$ → StmtAddress { file: cypher_dir/bare.cypher, stmt_name }
  else → BareName { name, resolved: cypher_dir/name.cypher }
```

**Cypherdoc extraction** (per statement in a file):
```
For each statement in the parsed file:
  look for the preceding sibling with kind "doc_comment"
  if found: parse text with tree-sitter-cypherdoc → CypherDoc
  attach to StatementEntry.doc
```

**Preflight Stage 3 with cypherdoc** (extended from M1):
```
If StatementEntry.doc is Some(doc):
  Required params = params with ParamDecl.required == true
  Optional params = params with ParamDecl.required == false (have defaults)
  Any required param not in ParamMap → error with cypherdoc appended
Else (no cypherdoc):
  Treat all $x refs as required (existing M1 behavior)
```

**`--describe` early return**: After building the statement queue (resolution +
cypherdoc parsing), if `args.describe` is true, print documentation for each
entry and `return Ok(())` before any preflight lint or Bolt connection.

**Parameter merge order** (M1 unchanged, M2 adds positional map as lowest
precedence):
```
1. Load --params FILE into map (lowest precedence)
2. Parse [PARAMS] map literal, merge (overrides --params on conflict)
3. Apply --param flags (highest precedence, overrides both)
```

Wait — RFC-003 says `--param` takes precedence over `[PARAMS]`, and `[PARAMS]`
and `--params` are mutually exclusive. So the actual merge order is:
```
base = either parse_map_literal([PARAMS]) or load_params_file(--params)  # mutually exclusive
for each --param flag: base.insert(key, value)  # --param overrides base
```

### Agent context update

The `<!-- SPECKIT START -->` block in `CLAUDE.md` should point to
`specs/003-query-library-params/plan.md` so agents load this plan as context.
