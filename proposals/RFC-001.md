---
number: "001"
title: "RFC Format and Project Goals"
status: "Accepted"
date: "2026-05-09"
authors:
  - "Andreas Kollegger <andreas.kollegger@neo4j.com>"
---

# RFC-001: RFC Format and Project Goals

## Summary

This RFC defines the format used for all RFCs in the `relate` project and describes
the project's goals, planned subcommands, and technology choices. It serves as both
the meta-RFC and the founding document.

## Motivation

The `relate` CLI provides developer tooling for working with Cypher and Gram files
alongside Neo4j graph databases. A structured RFC process ensures that new features
are motivated, specified, and traceable before implementation begins.

The RFC-driven approach integrates with GitHub speckit to move from RFC to
implementation systematically. Each RFC maps to a speckit workflow that generates
a detailed implementation plan and task breakdown.

## Design

### RFC Format

All RFCs in this repository follow this format:

**Filename:** `proposals/RFC-NNN.md` where `NNN` is a zero-padded 3-digit number
(e.g. `RFC-001`, `RFC-042`).

**Frontmatter** (YAML between `---` delimiters):

| Field | Type | Description |
|-------|------|-------------|
| `number` | string | 3-digit zero-padded (e.g. `"001"`) |
| `title` | string | Short descriptive title |
| `status` | string | `Draft`, `Accepted`, `Rejected`, or `Superseded` |
| `date` | string | ISO 8601 date of last status change (`YYYY-MM-DD`) |
| `authors` | list | Author strings in `"Name <email>"` format |

**Required sections** (all H2):

1. **Summary** — one paragraph description of the proposal
2. **Motivation** — why this change is needed; what problem it solves
3. **Design** — the substance: technical decisions, CLI interfaces, file layouts,
   data models, trade-offs
4. **Unresolved Questions** — open issues for implementors to resolve

### Project Goals

`relate` is a Rust CLI tool for:

- **Linting** `.cypher` and `.gram` files (and inline snippets via cypherdoc)
- **Writing** `.gram` data to a Neo4j database over Bolt
- **Reading** Neo4j query results back as `.gram` files
- **Hosting** parameterized `.cypher` files as MCP tools over stdio
- **Parsing** `.cypher` and `.gram` files to display their syntax trees
  (adopting the tree-sitter CLI convention)

Non-goals (at bootstrap):
- No GUI or web interface
- No schema validation beyond what the grammars enforce
- No multi-database or multi-cluster management

### CLI Interface

```
relate [OPTIONS] <COMMAND>

Commands:
  lint    Lint .cypher or .gram files and snippets
  parse   Parse a file and display its syntax tree
  write   Write .gram files to Neo4j
  read    Read Neo4j results and save as .gram
  mcp     Host a directory of parameterized .cypher files as MCP tools (stdio)
  help    Print help information
```

The `parse` command adopts the tree-sitter convention: default output is an
S-expression syntax tree; `--format json` is available for machine consumption.

Neo4j credentials for `write` and `read`:
- `--uri bolt://localhost:7687` (default)
- `--user neo4j` (default)
- `--password <value>` or `NEO4J_PASSWORD` env var

### Technology Stack

| Component | Choice | Rationale |
|-----------|--------|-----------|
| Language | Rust (edition 2021, MSRV 1.80.0) | Performance, safety, strong ecosystem |
| CLI framework | clap v4 (derive API) | Ergonomic, well-maintained, generates help text |
| Async runtime | tokio (full features) | Required by neo4rs and rmcp |
| Neo4j driver | neo4rs 0.9 RC | Async Bolt protocol; most current Rust driver |
| Error handling | anyhow | Flexible `?`-propagation in a CLI context |
| Gram grammar | tree-sitter-gram 0.3.7 (crates.io) | Published, stable |
| Cypher grammar | tree-sitter-cypher 0.2 (crates.io) | Includes cypherdoc sub-grammar |
| MCP | rmcp 1.6 (stdio transport) | Clean Rust SDK for Model Context Protocol |

### Grammar Dependencies

The project uses two tree-sitter grammars from [gram-data](https://github.com/gram-data):

- **tree-sitter-gram** — parses `.gram` files (property graph pattern notation)
- **tree-sitter-cypher** — parses `.cypher` files; includes the **cypherdoc**
  sub-grammar for extracting Cypher snippets from documentation (Markdown code blocks)

Both grammars are published on crates.io.

The companion CLI tool `cypher-data` (binary: `cypher`) is also published but is
binary-only — `relate` adopts its conventions rather than depending on it as a library:
- `-e`/`--expr` for inline expression linting
- `--json` for machine-readable output
- `--strict` to treat warnings as errors

### Subcommand Roadmap

| Subcommand | RFC | Status |
|------------|-----|--------|
| `lint` | RFC-002 | Planned |
| `parse` | RFC-003 | Planned |
| `write` | RFC-004 | Planned |
| `read` | RFC-005 | Planned |
| `mcp` | RFC-006 | Planned |

### Speckit Integration

Each RFC maps to a speckit workflow. The naming convention for feature branches
is `NNN-short-name` matching the RFC number (e.g. `002-lint`, `006-mcp`).

The typical workflow for each feature:
1. Draft RFC in `proposals/RFC-NNN.md`
2. Iterate on the RFC until `status: Accepted`
3. Kick off speckit workflow pointing at the RFC
4. Speckit generates implementation plan and task breakdown
5. Implement, test, and merge

## Decisions (resolved after initial draft)

- **tree-sitter-cypher**: Depend on `tree-sitter-cypher = "0.2"` from crates.io.
  Do not depend on `cypher-data` (binary-only); adopt its conventions in `relate lint`.

- **neo4rs**: Pin to `0.9.0-rc.9` RC for the most current async Bolt API.
  Revisit when 0.9 reaches stable.

- **Neo4j connection args**: Defined as global args (`--uri`, `--user`, `--password`)
  flattened into the root `Cli` struct via `Neo4jArgs`. Password is `Option<String>`;
  commands that require it call `neo4j.require_password()` for lazy validation.
  Commands like `lint` and `parse` are never blocked by missing credentials.
