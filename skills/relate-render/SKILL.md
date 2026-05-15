---
name: relate-render
description: >
  Visualize a .gram file as an interactive HTML page (pan/zoom, click-to-inspect)
  or a static SVG diagram using `relate render`. Use this skill whenever you need
  to render, preview, or embed a gram graph.
triggers:
  - "render gram"
  - "visualize gram"
  - "relate render"
  - "graph to svg"
  - "graph to html"
  - "render graph"
---

# Skill: relate render

> **Purpose**: Convert a `.gram` graph source file into an interactive HTML
> visualization or a static SVG diagram.

---

## Workflow

### Step 1 — Identify the gram file

Confirm the path to the `.gram` file to render.

| Situation | Action |
|-----------|--------|
| File path given | Use it directly |
| Inline gram source | Write it to a temp file first: `echo '(a)-[:R]->(b)' > /tmp/graph.gram` |
| Need to find gram files | `find . -name "*.gram"` |

**Checkpoint**: You have an absolute or relative path to a readable `.gram` file.

---

### Step 2 — Choose output format

| Goal | Command |
|------|---------|
| Interactive browser visualization (default) | `relate render graph.gram` |
| Static SVG for docs/README embed | `relate render graph.gram --format svg` |

The default format is `html`. HTML output includes pan/zoom, node click-to-inspect,
and colored path envelopes. SVG output is deterministic and embeds directly in Markdown.

**Checkpoint**: You know which format to produce.

---

### Step 3 — Run relate render

```bash
# Interactive HTML (default) — writes graph.html next to graph.gram
relate render graph.gram

# Static SVG
relate render graph.gram --format svg

# Explicit output path
relate render graph.gram --output /tmp/out.html

# Auto-open in default viewer after writing
relate render graph.gram --open

# Machine-readable output path (for scripting)
relate render graph.gram --json
```

**Checkpoint**: The command exits 0 and the output file exists at the expected path.

---

### Step 4 — Verify the output

| Format | Verification |
|--------|-------------|
| HTML | Open in a browser (offline); confirm graph is visible; click a node to inspect properties |
| SVG | Embed in a Markdown README; confirm it renders on GitHub or in a Markdown preview |

To verify SVG determinism (required for CI):
```bash
relate render graph.gram --format svg --output /tmp/a.svg
relate render graph.gram --format svg --output /tmp/b.svg
diff /tmp/a.svg /tmp/b.svg   # must produce no output
```

**Checkpoint**: The output file opens/embeds correctly and shows the expected graph structure.

---

### Exit criteria

Workflow is complete when:
- `relate render` exits `0`
- The output file exists and is openable or embeddable
- SVG renders are verified deterministic if they will be committed to the repo

---

## Exit codes

| Code | Meaning | Next action |
|------|---------|-------------|
| `0` | Success — output file written | Proceed |
| `1` | Render error (parse failure, empty graph) | Check stderr; fix the `.gram` source |
| `2` | I/O error (unreadable input, unwritable output) | Check file paths and permissions |

---

## Anti-Rationalization Table

| Excuse | Rebuttal |
|--------|----------|
| "I can read the gram source directly, I don't need to render it." | A graph visualization reveals structural patterns (paths, clusters, cycles) that are invisible in the raw text. |
| "The SVG is too large to embed in the README." | Reduce the gram file to the relevant subgraph. `relate render` renders exactly what you give it. |
| "I'll skip the determinism check — the SVG looks the same." | Golden-file tests catch layout drift introduced by dependency upgrades. Visual inspection is not sufficient. |
| "HTML output requires a server to view." | The HTML file is fully self-contained and opens from `file://` with no server. |
| "I'll use `--open` in CI." | `--open` is for interactive use only. In CI, use `--json` to capture the output path and verify the file was written. |

---

## Quick reference

```bash
# Default HTML render
relate render graph.gram

# SVG for documentation
relate render graph.gram --format svg

# Write to explicit path and open
relate render graph.gram --output /tmp/preview.html --open

# Scripting: get output path as JSON
OUTPUT=$(relate render graph.gram --json | jq -r .output)

# Render gram source from stdin via temp file
echo '(alice:Person)-[:KNOWS]->(bob:Person)' > /tmp/g.gram && relate render /tmp/g.gram
```
