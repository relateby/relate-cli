# Quickstart: Implementing `relate render`

**Branch**: `008-gram-render` | **Date**: 2026-05-14

Implementation order is chosen so each step produces a runnable, testable
milestone. Steps 1–4 are the P1 deliverable (HTML output). Steps 5–6 are the P2
deliverable (SVG output). Steps 7–8 are the constitution-required additions.

---

## Step 1 — Add `RenderArgs` to `src/cli.rs`

Add a `Render` variant to the `Commands` enum and define `RenderArgs`:

```rust
#[derive(Args)]
pub struct RenderArgs {
    pub file: PathBuf,

    #[arg(long, default_value = "html", value_enum)]
    pub format: OutputFormat,

    #[arg(long, short = 'o')]
    pub output: Option<PathBuf>,

    #[arg(long)]
    pub open: bool,

    #[arg(long)]
    pub json: bool,
}

#[derive(ValueEnum, Clone)]
pub enum OutputFormat { Html, Svg }
```

Wire `Commands::Render(args)` into `main.rs` dispatch (sync call, no `.await`
— render is a pure computation).

**Checkpoint**: `cargo build` compiles. `relate render --help` shows all flags.

---

## Step 2 — Create `src/gram_render/` skeleton

Create these files with stub implementations:

- `src/gram_render/mod.rs` — re-exports `parse_gram`, `render_html`, `render_svg`,
  all public types.
- `src/gram_render/graph.rs` — `GramGraph`, `GramNode`, `GramEdge`, `GramPath`,
  `PathMember`, `NestingRelation`, `RenderError`. `parse_gram` returns
  `RenderError::EmptyGraph` for all inputs (stub).
- `src/gram_render/layout.rs` — `LayoutResult`, `Vec2`. `compute()` returns all
  nodes at position (0, 0) (stub).
- `src/gram_render/html.rs` — `render_html()` returns a minimal `"<html></html>"`
  (stub).
- `src/gram_render/svg.rs` — `render_svg()` returns `"<svg/>"` (stub).

**Checkpoint**: `cargo test` compiles. Stubs return without panic.

---

## Step 3 — Implement `parse_gram` in `graph.rs`

Walk the tree-sitter-gram parse tree to populate `GramGraph`. Key traversal
targets (node kinds from the tree-sitter-gram grammar):
- `node_pattern` → `GramNode`
- `relationship_pattern` → `GramEdge` (check `directed` from child node kind)
- `path_pattern` → `GramPath` with `PathMember` sequence
- `nested_graph` → `NestingRelation`

Re-use the existing `tree-sitter-gram` integration already in `src/commands/`
for reference on how to traverse the tree.

**Checkpoint**: `cargo test -- gram_render::graph` passes. Write unit tests for
a gram string with at least: one node, one directed edge, one named path, one
nested structure.

---

## Step 4 — Implement `render_html` (HTML path, P1 complete)

4a. **Vendor JS assets**: Place `paper-core.min.js` and `d3-force.min.js` under
`assets/vendor/`. Add `include_str!` constants in `html.rs`:
```rust
const PAPER_JS: &str = include_str!("../../assets/vendor/paper-core.min.js");
const D3_FORCE: &str = include_str!("../../assets/vendor/d3-force.min.js");
```

4b. **Serialize `GramGraph` to JSON** using `serde::Serialize` derives on all
types. Add the `layout` field to the JSON by calling `layout::compute()` before
serialization.

4c. **Write the HTML template** in `html.rs` as a Rust format string (or
`include_str!` from `assets/templates/render.html`). The template structure:

```html
<!DOCTYPE html>
<html>
<head><meta charset="utf-8"><title>gram</title>
<style>/* minimal: full-viewport canvas, sidebar */</style>
</head>
<body>
<canvas id="gram-canvas"></canvas>
<pre id="sidebar"></pre>
<script>{PAPER_JS}</script>
<script>{D3_FORCE}</script>
<script type="application/json" id="gram-data">{DATA_JSON}</script>
<script>{RENDERING_JS}</script>
</body>
</html>
```

4d. **Write `rendering.js`** (checked in under `assets/templates/`):
- Parse `gram-data` JSON.
- Initialize Paper.js canvas.
- For each gram path: compute convex hull of member node positions, draw
  semi-transparent `Path` with Bézier rounding, assign distinct hue.
- Draw edges as `Path` objects with arrowheads (directed) or plain (undirected).
- Draw nodes as `Group` (circle + label).
- Draw compound node outlines from `Group.bounds`.
- Wire pan/zoom event handlers.
- Wire `project.hitTest()` for click-to-inspect → sidebar.

**Checkpoint**: `relate render sample.gram` produces a `.html` file that opens in
a browser and shows the graph. Named paths have distinct colored envelopes.

---

## Step 5 — Implement `layout::compute` (spring embedder)

```rust
pub fn compute(graph: &GramGraph) -> LayoutResult {
    // 1. Place nodes on a circle (deterministic initial positions)
    // 2. 500 iterations:
    //    a. Repulsion: F = k_r / dist^2 between all pairs
    //    b. Attraction: F = k_a * (dist - rest_len) along edges
    //    c. Centering: nudge each node toward (0,0)
    //    d. Scale step by cooling factor (1.0 → 0.1 linearly)
    // 3. Normalize positions to fit a 600×600 canvas with padding
}
```

For nested subgraphs, call `compute` recursively on the children and translate
the result into the parent's bounding area.

**Checkpoint**: `cargo test -- gram_render::layout` passes. A 10-node ring graph
produces positions that are visually distributed (assert no two nodes < 20px apart).

---

## Step 6 — Implement `render_svg` (SVG path, P2 complete)

Use kurbo types and the svg crate:

```rust
fn render_svg(graph: &GramGraph) -> String {
    let layout = layout::compute(graph);
    let mut doc = svg::Document::new().set("viewBox", …);
    
    // Layer 1: path envelopes (convex hull → BezPath → svg::node::element::Path)
    // Layer 2: edges (Line/CubicBez → svg path; arrowhead marker in <defs>)
    // Layer 3: nodes (Circle / RoundedRect)
    // Layer 4: compound outlines (RoundedRect from Group equivalent)
    // Layer 5: labels (svg Text elements)
    
    doc.to_string()
}
```

Golden-file tests: for each `.gram` fixture in `tests/render_svg/fixtures/`,
compare output against the checked-in `.svg` expected file.

**Checkpoint**: `relate render sample.gram --format svg` produces a valid SVG.
Running twice produces byte-identical output. Golden-file tests pass.

---

## Step 7 — Wire `--json` flag and exit codes

In `src/commands/render.rs`:
- On success with `--json`: print `{"output":"…","format":"…"}` to stdout.
- On `RenderError`: print `{"error":"…"}` to stdout (with `--json`) or to stderr
  (without). Exit code 1 for parse errors, 2 for I/O errors.

**Checkpoint**: `relate render sample.gram --json | jq .output` prints the file path.

---

## Step 8 — Add MCP tool and SKILL.md

8a. In `src/commands/mcp.rs`, register `render_gram` tool:
- Input schema: `gram_source` (string), `format` (string, default `"svg"`).
- Handler: call `gram_render::parse_gram()` then `render_svg()` or `render_html()`.
- Return content array with the rendered string.

8b. Create `skills/relate-render/SKILL.md` with:
- YAML frontmatter: `name`, `description`, `triggers`.
- Numbered workflow: (1) identify gram file, (2) run `relate render`, (3) verify
  output, (4) open or embed.
- Exit criteria: output file written and openable.
- Anti-rationalization table.

8c. Update `skills/relate/SKILL.md` routing table to include `render`.

**Checkpoint**: `relate mcp` exposes `render_gram` in the MCP tool list.
`skills/relate-render/SKILL.md` exists with all required sections.

---

## Dependency additions (`Cargo.toml`)

```toml
kurbo       = "0.11"
svg         = "0.17"
geo         = { version = "0.28", default-features = false, features = ["convex_hull"] }
open        = "5"
# serde_json already present
```

No new async dependencies. `render` is a sync command (`fn run`, no `.await`).
