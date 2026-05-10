<!-- SPECKIT START -->
For additional context about technologies to be used, project structure,
shell commands, and other important information, read the current plan
at specs/003-query-library-params/plan.md
<!-- SPECKIT END -->

# relate CLI

`relate` is a Rust CLI for working with `.cypher` and `.gram` files and Neo4j.
Binary name: `relate`. Single-crate (no workspace).

## Common Commands

```bash
cargo build
cargo test
cargo clippy --all-targets
cargo fmt --check
./target/debug/relate --help
```

## Architecture

```
src/
├── main.rs              # #[tokio::main], top-level dispatch
├── cli.rs               # all clap structs (Cli, Commands, *Args)
└── commands/
    ├── mod.rs           # shared helpers (from_cypher_diagnostic)
    ├── lint.rs          # sync
    ├── parse.rs         # sync
    ├── query.rs         # async (neo4rs) — parameterized Cypher execution
    ├── write.rs         # async (neo4rs)
    ├── read.rs          # async (neo4rs)
    └── mcp.rs           # async (rmcp stdio)
proposals/
└── RFC-NNN-short-name.md  # RFCs for each feature
```

## Agent Skills

Skills live in `skills/` at the repo root, one directory per skill, each containing
a `SKILL.md` file (agentskills.io / `npx skills find` convention). Each skill is a
workflow-driven runbook — steps, checkpoints, exit criteria, anti-rationalization table.

```
skills/
├── relate/SKILL.md          # overview + install (load this first)
├── relate-lint/SKILL.md     # lint workflow
└── relate-query/SKILL.md    # query workflow (parameterized Cypher execution)
```

## RFC Convention

- 3-digit numbering: RFC-001, RFC-002, ...
- Frontmatter: `number`, `title`, `status`, `date`, `authors`
- Sections: Summary, Motivation, Design, Unresolved Questions
- Each RFC maps to a speckit feature branch named `NNN-short-name`

## Key Dependencies

- **clap 4** (derive API) — all arg structs live in `src/cli.rs`
- **tokio** (full) — `main` is async; sync commands are called without `.await`
- **neo4rs 0.9.0-rc.9** — Bolt driver; used in `write`, `read`, and `query` commands
- **tree-sitter-cypher 0.2** — from crates.io; includes cypherdoc sub-grammar; root node kind: `source_file`, statement kind: `statement`, parameter kind: `parameter`
- **tree-sitter-gram 0.3.7** — from crates.io
- **comfy-table 7** — Unicode table rendering for `query` output
- **rmcp 1.6** — MCP stdio server; used only in `mcp` command

## Neo4j Credentials

Never hardcode credentials. Use `--password` flag or `NEO4J_PASSWORD` env var.
