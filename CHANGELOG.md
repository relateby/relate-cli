# Changelog

All notable changes to `relate` are documented here.

The format is based on [Keep a Changelog](https://keepachangelog.com/en/1.1.0/).
Versions follow [Semantic Versioning](https://semver.org/spec/v2.0.0.html).

## [Unreleased]

## [0.1.0] - TBD

### Added
- `relate lint` — validate `.cypher` and `.gram` files with ariadne diagnostics
- `relate parse` — parse `.cypher` / `.gram` files and print the syntax tree
- `relate query` — execute parameterized Cypher against a Neo4j database
- `relate query --apply` — batch-execute a Cypher template against CSV / JSON / JSONL input
- `relate mcp` — expose relate commands as an MCP stdio server
- Query library resolution (`~/.relate/queries/`) and inline parameter binding
- Cypherdoc support: extract and run named query blocks from `.cypher` files
