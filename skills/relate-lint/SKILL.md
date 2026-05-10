---
name: relate-lint
description: >
  Lint Cypher and Gram files (and embedded code fences in Markdown/AsciiDoc) using
  `relate lint`. Use this skill whenever you need to validate .cypher, .gram, .md, or
  .adoc files before writing them to Neo4j, committing them, or using them in queries.
triggers:
  - "lint cypher"
  - "check cypher"
  - "validate gram"
  - "lint before write"
  - "relate lint"
---

# Skill: relate lint

> **Purpose**: Validate Cypher and Gram sources. Catch syntax errors and structural
> warnings before they reach the database or break documentation.

---

## Workflow

### Step 1 — Choose input mode

| Situation | Command |
|-----------|---------|
| One or more files | `relate lint file.cypher file2.gram` |
| Entire project directory | `relate lint .` or `relate lint <dir>` |
| Inline snippet | `relate lint --expr "MATCH (n:Person) RETURN n"` |
| Piped from another tool | `echo "MATCH (n) RETURN n" \| relate lint` |
| Markdown/AsciiDoc docs | `relate lint docs/` |

**Checkpoint**: You know which mode to use. If the source is a string in conversation context, use `--expr`. If it is a file path, use the file. If multiple files need checking together, pass the directory.

---

### Step 2 — Choose output format

- **Human review**: omit `--json` (default — ariadne-rendered output with source annotations)
- **Programmatic/CI use**: add `--json` to get a structured array you can parse

**Checkpoint**: You have decided whether downstream processing needs JSON.

---

### Step 3 — Set strictness

- Without `--strict`: only `Error`-severity diagnostics cause exit code 1.
- With `--strict`: any `Warning` also causes exit code 1. Use this in CI or before a `relate write`.

**Checkpoint**: You know the required quality bar for this context.

---

### Step 4 — Run and interpret exit code

| Exit code | Meaning | Next action |
|-----------|---------|-------------|
| `0` | No errors (no warnings if `--strict`) | Proceed |
| `1` | Lint findings present | Read diagnostics, fix source, re-run |
| `2` | Tool failure (bad file type, I/O error) | Check the file path and extension |

**Checkpoint**: Exit code is captured. If `1`, continue to Step 5. If `0`, workflow is complete.

---

### Step 5 — Read and act on diagnostics

Each diagnostic reports:
- **severity** (`error` / `warning` / `information` / `hint`)
- **rule** identifier (e.g., `UnlabelledNode`, `ParseError`)
- **message** — human-readable explanation
- **file** and **range** (line/column, 0-based)

For `--json` output, the schema is:
```json
[
  {
    "severity": "warning",
    "rule": "UnlabelledNode",
    "message": "Node pattern without a label causes a full node scan.",
    "file": "queries/find-all.cypher",
    "range": { "start": { "line": 0, "column": 6 }, "end": { "line": 0, "column": 8 } }
  }
]
```

Fix each `error` before proceeding. Fix `warning` items if `--strict` is required or if code quality warrants it.

**Checkpoint**: All `error`-severity diagnostics are resolved. Re-run to confirm exit code 0.

---

### Exit criteria

Workflow is complete when:
- `relate lint` exits `0` for the target files/directory
- OR you have documented a deliberate exception (e.g., a warning suppressed by design) and the caller approves

---

## Anti-Rationalization Table

| Excuse | Rebuttal |
|--------|----------|
| "The query looks fine visually, I'll skip linting." | Parser-level errors and unlabelled node patterns are invisible to the eye. Lint takes under 2 seconds. |
| "It's just a draft, I'll lint later." | Lint-before-write is the cheapest point to catch errors. Fixing them after a `relate write` requires cleaning up graph data. |
| "The file has no `.cypher` extension so lint won't help." | Markdown and AsciiDoc files with fenced code blocks are fully supported. `relate lint docs/` lints embedded snippets too. |
| "I don't want to add `--strict` because it makes CI stricter." | That is the point. `UnlabelledNode` warnings cause full-node scans in production. Strict mode is the right default for any query that reaches Neo4j. |
| "I only need the JSON output for a script, I'll parse stdout myself." | Use `--json`. The schema is stable and documented. Hand-parsing ariadne output is fragile. |

---

## Quick reference

```bash
# Lint a single file
relate lint query.cypher

# Lint everything in a project
relate lint .

# Check an inline snippet
relate lint --expr "MATCH (n:Person)-[:KNOWS]->(m) RETURN n, m"

# Strict mode (warnings = errors)
relate lint --strict .

# JSON output for CI or scripting
relate lint --json queries/ | jq '.[].rule'

# Select engine for stdin
echo "(a)-->(b" | relate lint --lang gram
```

## Supported file types

| Extension | Engine |
|-----------|--------|
| `.cypher` | cypher-data |
| `.gram` | gram-data |
| `.md` | Cypher + Gram fences extracted and linted |
| `.adoc` | `[source,cypher]` and `[source,gram]` blocks extracted and linted |
