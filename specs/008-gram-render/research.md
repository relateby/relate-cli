# Research: Gram Visualization

**Branch**: `008-gram-render` | **Date**: 2026-05-14

## Decisions

---

### 1. HTML rendering library

**Decision**: Paper.js (core bundle, ~220 KB) + d3-force (~45 KB), both bundled inline.

**Rationale**: Paper.js provides a compositional scene graph with first-class
`Group.bounds` (exact bounding box of grouped children, needed for compound node
outlines), boolean path operations, and SVG export — all with zero runtime
dependencies and a `file://`-compatible CDN URL. d3-force supplies force-directed
layout without requiring the full D3 bundle. The combined 265 KB payload is
acceptable for an interactive diagram file.

**Alternatives considered**:
- Cytoscape.js: better out-of-box graph features but forces all rendering through
  its own style system; compound node layout is optimizer-driven and cannot express
  gram's path envelopes as first-class visual shapes.
- Penrose: declarative constraint system is elegant for code generation but
  produces nondeterministic layouts and requires CDN + HTTP server — incompatible
  with the `file://` requirement.
- D3.js alone: no native compound node concept; requires significant custom code
  to replicate Paper.js's Group.bounds behavior.

---

### 2. SVG rendering approach

**Decision**: Pure Rust pipeline — kurbo 0.11 (geometry) + svg 0.17
(serialization) + geo 0.28 (convex hull) + custom spring embedder (~100 lines).

**Rationale**: Eliminates all external runtime dependencies for the SVG path.
kurbo is the Linebender project's production 2D geometry library (used by Xilem,
Vello, and Google's font toolchain); its `BezPath`, `CubicBez`, `RoundedRect`, and
affine transform types cover every shape needed. The svg crate provides a clean
element builder and serializer. geo's `ConvexHull` trait operates on a
`MultiPoint` with a single method call. A custom spring embedder avoids adding
`fdg-sim` as a dependency for ~100 lines of physics.

**Alternatives considered**:
- fdg-sim crate: well-maintained force simulation but adds a dependency for code
  we can write in ~100 lines. Listed as fallback if the custom embedder proves
  insufficient.
- tiny-skia: rasterizes to PNG, not SVG; eliminated.
- resvg + usvg: renders SVG, does not generate it; eliminated.
- Graphviz (DOT, system binary): would handle layout and SVG output but adds a
  required system dependency and doesn't support gram paths as first-class shapes.

---

### 3. JS asset bundling strategy

**Decision**: Vendor files checked into `assets/vendor/` and included at compile
time via `include_str!`. SHA-256 checksums recorded in `assets/vendor/CHECKSUMS`.

**Rationale**: Eliminates network dependency in CI, ensures reproducible builds,
and keeps `relate render` usable offline. The assets are small enough (265 KB
total) that checking them in is standard practice (cf. Bootstrap, htmx in other
projects).

**Alternatives considered**:
- `build.rs` download with pinned URL + SHA-256 verification: avoids storing
  binary blobs in git but requires network access during `cargo build`. Rejected
  because it breaks air-gapped builds and adds build complexity.
- CDN links in the HTML template: requires internet access to view output.
  Rejected (violates self-contained constraint).

---

### 4. Spring embedder implementation

**Decision**: Custom implementation (~100 lines in `src/gram_render/layout.rs`):
Coulomb repulsion between all node pairs + Hooke attraction along edges +
centring force. Fixed 500 iterations with linear cooling schedule. Runs
synchronously (no async, no threads).

**Rationale**: Sufficient for < 200 nodes (< 5 s target). No additional
dependency. Deterministic given a fixed initial layout seed (nodes placed on a
circle by index).

**Alternatives considered**:
- fdg-sim: production-quality force simulation but adds a crate for functionality
  we can replicate in 100 lines. Noted as upgrade path if needed.
- petgraph layout: no built-in force layout in petgraph; would still need custom
  physics.

---

### 5. Cross-platform file open (`--open`)

**Decision**: `open` crate (https://crates.io/crates/open). Single dependency,
uses `xdg-open` on Linux, `open` on macOS, `start` on Windows.

**Rationale**: Standard approach for this pattern in Rust CLI tools. Zero system
dependency beyond the OS's own file association mechanism.

---

### 6. Convex hull for path envelopes

**Decision**: `geo::algorithm::convex_hull::ConvexHull` trait on
`geo::MultiPoint<f64>`. Expand hull vertices radially from centroid by a padding
constant, then connect with `kurbo::CubicBez` (tangent-continuous) for smooth
enclosures.

**Rationale**: geo's convex hull is a single method call returning a
`geo::Polygon`. Converting vertices to kurbo points and applying Bézier smoothing
is ~20 lines. No need to implement gift-wrapping manually.

---

### 7. GramGraph type ownership

**Decision**: `GramGraph` lives in `src/gram_render/graph.rs`, owned by the
incubated library module. The tree-sitter traversal that populates it also lives
there. `src/commands/render.rs` calls `gram_render::parse_gram(&source)` and
never touches the tree-sitter API directly.

**Rationale**: Matches the extraction boundary — when `src/gram_render/` becomes
a standalone crate, the tree-sitter-gram dependency moves with it, not with the
CLI.

---

### 8. MCP tool design for `render_gram`

**Decision**: The MCP tool `render_gram` accepts `gram_source: String` and
`format: "html" | "svg"` (default `"svg"`). Returns `{"output": "<svg or html
string>"}`. For HTML, returns the full HTML document as a string. SVG is preferred
for agent use because it is smaller and unambiguous.

**Rationale**: Agents (LLMs, documentation generators) want the rendered content
as a string, not a file path. The file-writing concern is a CLI concern only. The
library's `render_svg()` / `render_html()` return strings, so the MCP tool is a
thin adapter with no layout differences.
