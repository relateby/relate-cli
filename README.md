# relate

CLI for working with `.cypher` and `.gram` files and Neo4j.

## Status

Early development. Subcommands are stubs; see [`proposals/`](proposals/) for the RFC roadmap.

## Installation

```bash
cargo install --path .
```

## Usage

```
relate --help

relate lint query.cypher
relate parse schema.gram
relate write --password $NEO4J_PASSWORD nodes.gram
relate read --password $NEO4J_PASSWORD "MATCH (n) RETURN n LIMIT 10"
relate mcp ./queries/
```

## Development

```bash
cargo build
cargo test
cargo clippy --all-targets
cargo fmt
```

## RFC Process

New features are proposed in `proposals/RFC-NNN.md`. See [RFC-001](proposals/RFC-001.md) for
the RFC format specification and project goals. Each RFC maps to a speckit feature branch
named `NNN-short-name`.
