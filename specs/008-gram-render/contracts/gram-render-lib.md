# Library API Contract: `gram_render`

**Branch**: `008-gram-render` | **Date**: 2026-05-14  
**Module**: `src/gram_render/mod.rs`  
**Future crate**: `gram-render` (crates.io, if/when extracted)

This is the public surface of the incubated rendering library. No function
below may depend on `clap`, `tokio`, or any other CLI-specific crate.
This contract is the stability boundary: callers (including `src/commands/render.rs`
and the MCP tool) must not reach into submodules.

---

## Public API

### `parse_gram`

```rust
pub fn parse_gram(source: &str) -> Result<GramGraph, RenderError>
```

Parses a gram source string using `tree-sitter-gram` and returns the
intermediate `GramGraph`. Fails with `RenderError::ParseError` if the source
contains syntax errors, or `RenderError::EmptyGraph` if no nodes are found.

**Guarantees**:
- Pure function; no I/O, no side effects.
- Deterministic: same input always produces structurally identical `GramGraph`.

---

### `render_html`

```rust
pub fn render_html(graph: &GramGraph) -> String
```

Returns a complete, self-contained HTML document as a UTF-8 string. The
document embeds Paper.js, d3-force, and the graph data JSON inline; it
requires no internet connection or web server to render correctly in a browser.

**Guarantees**:
- Pure function; no I/O.
- The returned string is valid HTML5.
- The document opens correctly from a `file://` URI in modern browsers
  (Chrome ≥ 90, Firefox ≥ 88, Safari ≥ 14).

---

### `render_svg`

```rust
pub fn render_svg(graph: &GramGraph) -> String
```

Returns a complete SVG document as a UTF-8 string. The SVG is generated
entirely in Rust with no external tools or runtimes.

**Guarantees**:
- Pure function; no I/O.
- Deterministic: the same `GramGraph` always produces the same SVG string.
- The returned string is valid SVG 1.1 with a `viewBox` attribute and no fixed
  `width`/`height`, suitable for embedding in HTML and Markdown.
- Named gram paths are rendered as distinct filled convex-hull envelopes.
- Nested subgraph structures are rendered with compound node outlines.

---

## Public Types

```rust
pub struct GramGraph { … }      // see data-model.md
pub struct GramNode { … }
pub struct GramEdge { … }
pub struct GramPath { … }
pub enum   PathMember { … }
pub struct NestingRelation { … }

pub enum RenderError {
    ParseError(String),
    EmptyGraph,
    LayoutError(String),
}
impl std::fmt::Display for RenderError { … }
impl std::error::Error for RenderError {}
```

---

## MCP Tool: `render_gram`

The MCP tool is a thin adapter over `render_svg` / `render_html` registered in
`src/commands/mcp.rs`. It is not part of the `gram_render` library; it lives in
the CLI layer.

**Tool name**: `render_gram`  
**Description**: Renders a gram source string as SVG or HTML and returns the result.

**Input schema**:
```json
{
  "gram_source": "string (required) — gram notation source text",
  "format": "string (optional, default: \"svg\") — \"svg\" or \"html\""
}
```

**Output** (on success):
```json
{
  "content": [
    {
      "type": "text",
      "text": "<svg xmlns=...>...</svg>"
    }
  ]
}
```

**Error** (on parse failure):
```json
{
  "isError": true,
  "content": [{"type": "text", "text": "parse error: ..."}]
}
```

---

## Stability commitment (incubation period)

During incubation in `relate-cli`, the `gram_render` API is **unstable**: minor
versions may make breaking changes within the module. Once extracted to a
published crate, semver applies. Call sites within `src/commands/render.rs` and
`src/commands/mcp.rs` are the only permitted callers during incubation; no other
module in `relate-cli` should depend on `gram_render` internals.
