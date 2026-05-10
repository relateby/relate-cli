---
name: relate
description: >
  Bootstrap skill for the `relate` CLI — a tool for working with Cypher (.cypher)
  and Gram (.gram) files and connecting to Neo4j. Use this skill first when any task
  involves relate, Cypher files, Gram files, or Neo4j graph operations. It installs
  the tool, orients you to its commands, and tells you which sub-skill to load next.
triggers:
  - "relate"
  - "cypher file"
  - "gram file"
  - "neo4j query"
  - "lint cypher"
  - "write to neo4j"
  - "read from neo4j"
  - "mcp cypher"
---

# Skill: relate (overview)

> **Purpose**: Orient yourself to the `relate` CLI and route to the right sub-skill.
> This is the entry point — load it first, then load the command-specific skill for
> the task at hand.

---

## Step 1 — Install `relate`

### From source (current)

```bash
git clone https://github.com/relateby/relate-cli
cd relate-cli
cargo build --release
# binary at: ./target/release/relate
```

### Via cargo install (once published to crates.io)

```bash
cargo install relate
```

### Verify

```bash
relate --help
```

Expected: help text listing `lint`, `parse`, `write`, `read`, `mcp` subcommands.

**Checkpoint**: `relate --help` exits 0 and lists subcommands.

---

## Step 2 — Confirm prerequisites

| Prerequisite | Required by | How to satisfy |
|---|---|---|
| Rust toolchain (1.80+) | Building from source | `rustup update stable` |
| Neo4j instance | `write`, `read` | Local via Docker or AuraDB |
| `NEO4J_PASSWORD` env var | `write`, `read` | `export NEO4J_PASSWORD=<password>` |
| `.cypher` / `.gram` files | `lint`, `parse` | Create or clone a project |

**Checkpoint**: You know which subcommand you need and whether its prerequisites are met.

---

## Step 3 — Route to the right sub-skill

| Task | Command | Sub-skill to load |
|---|---|---|
| Validate Cypher or Gram files | `relate lint` | `relate-lint` |
| Inspect parse tree of a file | `relate parse` | *(skill coming soon)* |
| Write Gram graph data to Neo4j | `relate write` | *(skill coming soon)* |
| Query Neo4j and save as Gram | `relate read` | *(skill coming soon)* |
| Expose Cypher files as MCP tools | `relate mcp` | *(skill coming soon)* |

**Checkpoint**: You have identified the sub-skill to load. Load it now before proceeding.

---

## Step 4 — Global flags (available on every subcommand)

```
--uri <URI>           Neo4j Bolt URI  [default: bolt://localhost:7687]
--user <USER>         Neo4j username  [default: neo4j]
--password <PASSWORD> Neo4j password  [env: NEO4J_PASSWORD]
```

These flags are optional for `lint` and `parse`, required for `write` and `read`.

---

## Exit criteria

This skill's job is done when:
- `relate --help` exits 0, and
- You have loaded the appropriate sub-skill for the task

---

## Anti-Rationalization Table

| Excuse | Rebuttal |
|---|---|
| "I'll just run the Cypher query directly against Neo4j without linting first." | `relate lint` takes under 2 seconds and catches parse errors and full-scan warnings before they hit the database. Load `relate-lint` and run it first. |
| "I don't need `relate` — I'll use the neo4j-driver directly." | `relate` handles file-based workflows (`.cypher`, `.gram`), documentation linting, and MCP tool exposure. The driver handles runtime queries. They are complementary, not alternatives. |
| "I don't have `NEO4J_PASSWORD` set, so I'll skip the write step." | Set it: `export NEO4J_PASSWORD=<password>`. `relate write` and `relate read` refuse to run without it by design — credentials must never be hardcoded. |
| "The sub-skill isn't written yet, I'll skip it." | Load this overview skill and use `relate --help` + `relate <command> --help` as the reference until the sub-skill exists. `--help` is self-contained. |

---

## Available skills

```
skills/
├── relate/SKILL.md          ← this file (overview + install)
└── relate-lint/SKILL.md     ← lint workflow for .cypher, .gram, .md, .adoc
```
