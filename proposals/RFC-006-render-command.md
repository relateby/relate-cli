---
number: "006"
title: "relate render — Gram Visualization"
status: "Draft"
date: "2026-05-14"
authors:
  - "Andreas Kollegger <andreas.kollegger@neo4j.com>"
---

# RFC-006: relate render — Gram Visualization

## Summary

`relate render` produces visual output from `.gram` files. It supports two output
formats: a standalone HTML file for interactive browser-based exploration, and a
static SVG file generated entirely within the Rust process. Both formats require no
external runtime dependencies beyond a web browser (HTML) or nothing at all (SVG).

The HTML output uses **Paper.js** for composable vector shape rendering and
**d3-force** for physics-based graph layout, with both libraries bundled inline.
The SVG output uses **kurbo** for 2D geometry and a force-directed layout
implemented in Rust, producing deterministic, reproducible files suitable for
documentation and CI pipelines.

The rendering logic is incubated inside `relate-cli` but isolated behind a clean
library API with no CLI dependencies. The intent is that it graduates to a
standalone `gram-render` crate — or becomes a contribution to the upstream
`gram-data` ecosystem — once the design stabilises. `relate render` remains a
thin dispatch layer: read file, call library, write output.

## Motivation

Gram notation expresses structures that standard graph visualization tools handle
poorly or not at all:

- **First-class paths** — a gram file is a composition of named path expressions,
  not a flat edge list. Paths that share nodes need a visual treatment that
  distinguishes path membership from general graph topology.
- **Higher-order / nested structures** — gram allows a node's value to be a
  subgraph, creating compound nodes that contain their own nodes and edges.
- **Annotations** — properties assigned to elements after declaration can be
  visually distinguished from inline properties.
- **Undirected and mixed-direction edges** — gram supports undirected edges
  alongside directed ones in the same graph.

Graph-specific libraries (Cytoscape.js, vis-network) model compound nodes as a
built-in concept but force everything through their layout engine and style system,
limiting visual expressiveness for paths and annotations. Composable vector shape
libraries allow building precisely the visual metaphors gram's structures deserve.

A `render` subcommand lowers the barrier to exploring gram files, makes gram a
practical format for diagram-as-code workflows, and supports documentation use
cases where a reproducible static image is required.

`relate-cli` is designed to stay thin: it orchestrates existing libraries rather
than owning domain logic. Rendering is a substantial enough feature — with its own
geometry, layout, and multi-format output concerns — that it belongs in a dedicated
library rather than growing inside the CLI. Incubating it here lets the design
evolve against real gram files before committing to a published crate API.

## Design

### CLI Interface

```
relate render [OPTIONS] <FILE>

Arguments:
  <FILE>   A .gram file to render

Options:
  --format <FORMAT>   Output format [default: html]
                      [possible values: html, svg]
  --output <FILE>     Output file path [default: <input-stem>.<format>]
  --open              Open the output file after rendering
  -h, --help          Print help
```

Examples:

```bash
relate render graph.gram                    # → graph.html, opens in browser
relate render graph.gram --format svg       # → graph.svg, static file
relate render graph.gram --output out.html  # explicit output path
relate render graph.gram --open             # render + open automatically
```

### Gram → Internal Representation

Before either rendering path, the gram file is parsed by `tree-sitter-gram` into
an intermediate `GramGraph` structure:

```rust
struct GramGraph {
    nodes:    Vec<GramNode>,
    edges:    Vec<GramEdge>,
    paths:    Vec<GramPath>,
    nested:   Vec<NestingRelation>,
}

struct GramNode {
    id:         String,
    labels:     Vec<String>,
    properties: IndexMap<String, Value>,
    is_nested:  bool,   // true when this node is a value of another node
}

struct GramEdge {
    id:         Option<String>,
    source:     String,
    target:     String,
    label:      Option<String>,
    properties: IndexMap<String, Value>,
    directed:   bool,
}

struct GramPath {
    id:      Option<String>,
    members: Vec<PathMember>,   // alternating node IDs and edge IDs
}

struct NestingRelation {
    parent:   String,           // node ID of the containing node
    children: Vec<String>,      // node IDs of the nested subgraph
}
```

This representation is shared by both output paths and is independent of any
visualization library.

### Output Format A: HTML (Paper.js + d3-force)

**Architecture:**

```
gram file
  → tree-sitter-gram parser
  → GramGraph
  → serde_json serialization  →  data JSON
  → HTML template             →  output.html
       ├── <script> paper-core.min.js  (embedded inline, ~220 KB)
       ├── <script> d3-force.min.js    (embedded inline, ~45 KB)
       ├── <script type="application/json" id="gram-data"> … </script>
       └── <script> rendering.js (fixed template, ~200 lines) </script>
```

The HTML file is fully self-contained and opens from a `file://` URI without a
web server or internet connection. No Node.js or other runtime is required.

**Embedded JS bundles:**

The Paper.js core bundle and the d3-force module are checked into the repository
as minified files under `assets/vendor/` and included at compile time via
`include_str!`. The combined overhead is approximately 265 KB, acceptable for an
interactive diagram file.

**JSON data schema:**

```json
{
  "nodes": [
    { "id": "n1", "labels": ["Person"], "properties": {"name": "Alice"} }
  ],
  "edges": [
    { "id": "e1", "source": "n1", "target": "n2",
      "label": "KNOWS", "directed": true }
  ],
  "paths": [
    { "id": "p1", "members": ["n1", "e1", "n2"] }
  ],
  "nested": [
    { "parent": "n3", "children": ["n4", "n5"] }
  ]
}
```

**Browser rendering sequence:**

1. d3-force simulation runs at startup with repulsion between all nodes, link
   forces along edges, and a centering force. The simulation runs synchronously
   to completion (alpha decay) before drawing.
2. Path envelopes are drawn first: for each gram path, the convex hull of its
   member node positions is computed, expanded outward by a padding constant,
   optionally rounded via Bézier tangent continuation, and filled with a
   semi-transparent color distinct per path.
3. Edges are drawn as Paper.js `Path` objects with arrowheads (directed) or
   plain lines (undirected). Edges belonging to a path inherit that path's stroke
   color.
4. Nodes are drawn as Paper.js `Group` objects: a `Path.Circle` (or `Path.Rectangle`
   for compound nodes) plus a `PointText` label, grouped so transforms apply
   together.
5. Compound nodes (gram nested structures) are drawn as rectangles whose bounds
   are the union of their children's `Group.bounds` plus padding, with children
   rendered inside using a secondary force simulation scoped to the subgraph.
6. Annotations are rendered as secondary label text near the annotated element,
   styled with reduced opacity to distinguish them from primary labels.

**Interactivity:**

Pan is implemented by translating `paper.view.center` on drag. Zoom is implemented
by scaling `paper.view.zoom` on scroll. Hit-testing via `project.hitTest()` enables
click-to-inspect: clicking a node or edge logs its properties to a sidebar `<pre>`
element in the HTML page.

**Visual encoding for gram features:**

| Gram feature | Visual treatment |
|---|---|
| Node labels | Text inside/below the node circle |
| Node properties | Shown in sidebar on click |
| Directed edge | Line with filled triangular arrowhead |
| Undirected edge | Plain line, no arrowhead |
| Edge label | Small text at edge midpoint |
| Named path | Semi-transparent convex hull envelope, distinct color |
| Nested subgraph | Rounded rectangle bounding children; children use inner layout |
| Annotation | Secondary text label, reduced opacity, offset from element |

### Output Format B: SVG (pure Rust)

**Architecture:**

```
gram file
  → tree-sitter-gram parser
  → GramGraph
  → force layout (Rust)
      → node positions Vec<(f64, f64)>
  → kurbo geometry
      → node shapes
      → edge paths with arrowheads
      → path envelope BezPaths
      → compound node outlines
  → svg crate serialization
  → output.svg
```

No browser, no JS runtime, no system binaries. The SVG is generated entirely
within the Rust process.

**Layout algorithm:**

A simple spring embedder is implemented directly (approximately 100 lines):

- Repulsive Coulomb force between all node pairs
- Attractive Hooke force along each edge
- Bounding-box centering force
- Fixed number of iterations (default 500) with a cooling schedule

For most gram files (tens to low hundreds of nodes), this is fast enough to run
synchronously. The `fdg-sim` crate (`force-directed-graph-simulator`) is the
fallback if a more tunable implementation is needed.

Nested subgraphs are laid out recursively: after the outer layout converges,
each compound node's children are laid out independently within the compound
node's assigned bounding area.

**Crate dependencies:**

| Crate | Purpose |
|---|---|
| `kurbo` | 2D curves, Bézier paths, affine transforms, bounding boxes |
| `svg` | SVG element construction and serialization |
| `geo` | Geometric algorithms — `ConvexHull` over a `MultiPoint` |

**Geometry for gram features:**

*Nodes:* `kurbo::Circle` for the node shape; `kurbo::Affine` for label placement.

*Directed edges:* A `kurbo::Line` from source boundary to target boundary (offset
from center by node radius along the direction vector). Arrowhead is a
`kurbo::BezPath` triangle at the target end, with vertices computed from the edge
direction vector rotated ±25°.

*Undirected edges:* Same as directed but with no arrowhead.

*Path envelopes:* Node center points for all path members are collected into a
`geo::MultiPoint`. `ConvexHull::convex_hull()` produces the hull polygon. The hull
is expanded outward (offset each vertex away from centroid) and the vertices are
connected with `kurbo::CubicBez` curves (tangent-continuous at each corner) to
produce smooth enclosures. Each path gets a distinct HSL fill color derived from
its index.

*Compound nodes:* The bounding boxes of all children (`kurbo::Rect`) are unioned.
A rounded rectangle (`kurbo::RoundedRect`) is drawn around the union with padding.

**SVG structure:**

```xml
<svg xmlns="…" viewBox="…">
  <defs>
    <marker id="arrow">…</marker>   <!-- reused arrowhead -->
  </defs>
  <g id="path-envelopes">…</g>     <!-- drawn first, behind everything -->
  <g id="edges">…</g>
  <g id="nodes">…</g>
  <g id="compound-outlines">…</g>
  <g id="labels">…</g>
</svg>
```

The SVG uses `viewBox` with no fixed `width`/`height`, so it scales to fit in any
context. A `preserveAspectRatio="xMidYMid meet"` attribute is included.

### Crate Boundary and Externalization Strategy

All rendering logic lives in a module that is deliberately structured to look like
a library crate from day one. `src/commands/render.rs` is the only file that
touches CLI concerns (argument structs, file I/O, error reporting to stderr). Every
module below it accepts and returns plain Rust types with no dependency on `clap`,
`tokio`, or any other relate-cli crate.

**Incubated public API** (the surface that will become the external crate's API):

```rust
// src/gram_render/mod.rs — no CLI types, no file I/O

pub fn parse_gram(source: &str) -> Result<GramGraph, RenderError>;
pub fn render_html(graph: &GramGraph) -> String;
pub fn render_svg(graph: &GramGraph)  -> String;

pub struct GramGraph { … }   // owns nodes, edges, paths, nesting
pub struct RenderError(…);
```

`src/commands/render.rs` is then exactly three steps:

```rust
pub fn run(args: &RenderArgs) -> anyhow::Result<()> {
    let source  = fs::read_to_string(&args.file)?;
    let graph   = gram_render::parse_gram(&source)?;
    let output  = match args.format {
        Format::Html => gram_render::render_html(&graph),
        Format::Svg  => gram_render::render_svg(&graph),
    };
    fs::write(&dest, output)?;
    if args.open { open::that(&dest)?; }
    Ok(())
}
```

**Extraction path:** When the API is stable, extracting to a standalone crate is a
`cargo new gram-render` followed by moving `src/gram_render/` into it. `Cargo.toml`
gains a path (and later registry) dependency. No refactoring of the calling code is
needed because the module boundary already matches the crate boundary.

**Upstream contribution target:** The natural long-term home is the `gram-data`
ecosystem (the JavaScript `@gram-data` packages and associated tooling). A Rust
`gram-render` crate would complement the existing `@gram-data/d3-gram` Observable
notebook renderer and give the ecosystem a CLI-friendly, offline-capable output
path. This RFC does not commit to that contribution — it only ensures the design
does not preclude it.

**What stays in relate-cli permanently:** The `src/commands/render.rs` dispatch
layer, the `--format`, `--output`, and `--open` flags, and the `open` crate
invocation for `--open`. These are CLI concerns and have no place in a library.

### Source Layout

```
src/commands/render.rs      # CLI only: arg handling, file I/O, calls gram_render::*
src/gram_render/            # incubated library — no CLI dependencies
├── mod.rs                  # pub API: parse_gram(), render_html(), render_svg()
├── graph.rs                # GramGraph + construction from tree-sitter output
├── layout.rs               # spring embedder; LayoutResult { positions, bounds }
├── html.rs                 # HTML template + JSON serialization
└── svg.rs                  # kurbo geometry + svg crate serialization
assets/vendor/
├── paper-core.min.js       # Paper.js core bundle, included via include_str!
└── d3-force.min.js         # d3-force standalone bundle, included via include_str!
```

### Dependency Additions to Cargo.toml

```toml
[dependencies]
# existing deps omitted for brevity
kurbo   = "0.11"
svg     = "0.17"
geo     = { version = "0.28", default-features = false, features = ["convex_hull"] }
serde_json = "1"   # already present for query command
```

`fdg-sim` is listed as an alternative if the custom spring embedder proves
insufficient; it is not a required dependency.

## Unresolved Questions

1. **Bundled JS assets:** Should `paper-core.min.js` and `d3-force.min.js` be
   checked into the repository or fetched from a CDN at build time via a
   `build.rs` script? Checking in avoids network dependencies in CI; fetching
   at build time keeps the repo smaller. A `build.rs` download with a pinned URL
   and SHA-256 verification is the likely answer, with the files also committed
   as a fallback.

2. **Nested layout depth:** The recursive force layout for compound nodes works
   for one or two levels of nesting. Deeply nested gram structures (rare in
   practice) may produce poor layouts without a more sophisticated algorithm such
   as ELK's hierarchical layout. A depth limit with a warning is a pragmatic
   first-pass solution.

3. **Path color assignment:** With more than ~8 named paths in a single gram file,
   automatically assigned hues begin to clash. A mechanism for user-supplied
   colors (e.g., a `render:color` annotation on path elements) should be
   considered but is deferred.

4. **Layout algorithm flag:** A `--layout` option (`force`, `hierarchical`,
   `circular`) would improve usability for gram files with known structure (e.g.,
   tree-shaped graphs render better with a hierarchical layout). The spring
   embedder is the only layout in scope for the initial implementation.

5. **Large files:** Gram files describing graphs with hundreds of nodes will
   produce cluttered layouts regardless of algorithm. A `--max-nodes` guard with
   a warning, or a `--layout none` flag that emits raw structure without
   positioning, may be needed.

6. **Annotation rendering:** Gram annotations (properties assigned post-declaration
   via `@` syntax) are not currently extracted by the `tree-sitter-gram` parser
   in relate-cli's usage. Whether to surface them visually depends on parser
   support being added first.

7. **Externalization target:** Three paths exist for the incubated library once the
   API stabilises: (a) publish `gram-render` as a standalone crate on crates.io;
   (b) contribute it to the `gram-data` GitHub organisation alongside the existing
   JavaScript packages; (c) keep it as a relate-cli internal if no external
   consumers emerge. The decision should be deferred until at least one non-CLI
   consumer (e.g., a documentation generator or a language server) wants to depend
   on it directly. At that point the extraction cost is low by design.
