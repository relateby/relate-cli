# Data Model: Gram Visualization

**Branch**: `008-gram-render` | **Date**: 2026-05-14

All types live in `src/gram_render/`. None depend on CLI types.

---

## GramGraph

The central intermediate representation. Built from the tree-sitter parse tree by
`gram_render::parse_gram()` and consumed by both rendering paths.

```rust
pub struct GramGraph {
    pub nodes:   Vec<GramNode>,
    pub edges:   Vec<GramEdge>,
    pub paths:   Vec<GramPath>,
    pub nested:  Vec<NestingRelation>,
}
```

**Invariants**:
- `GramEdge.source` and `GramEdge.target` always reference an `id` present in
  `nodes`.
- `GramPath.members` alternates node ID / edge ID starting and ending with a node
  ID (gram path grammar guarantee).
- `NestingRelation.parent` references a node ID in `nodes`;
  `NestingRelation.children` reference node IDs in `nodes`.
- A node may appear as a child in at most one `NestingRelation`.

---

## GramNode

```rust
pub struct GramNode {
    pub id:         String,
    pub labels:     Vec<String>,
    pub properties: IndexMap<String, serde_json::Value>,
    pub is_nested:  bool,   // true when this node is a child in a NestingRelation
}
```

`IndexMap` preserves the declaration order of properties for stable JSON
serialization (HTML path) and deterministic SVG output.

---

## GramEdge

```rust
pub struct GramEdge {
    pub id:         Option<String>,
    pub source:     String,
    pub target:     String,
    pub label:      Option<String>,
    pub properties: IndexMap<String, serde_json::Value>,
    pub directed:   bool,   // false for undirected (~>) gram syntax
}
```

---

## GramPath

```rust
pub struct GramPath {
    pub id:      Option<String>,
    pub members: Vec<PathMember>,
}

pub enum PathMember {
    Node(String),   // node id
    Edge(String),   // edge id (only edges with an explicit id are tracked)
}
```

Anonymous edges (no id) within a path are represented by their implicit position
between the surrounding node members. For path envelope rendering, only the node
members are needed to compute the convex hull.

---

## NestingRelation

```rust
pub struct NestingRelation {
    pub parent:   String,         // node id of the containing node
    pub children: Vec<String>,    // node ids of the nested subgraph's nodes
}
```

---

## LayoutResult

Produced by `gram_render::layout::compute()`. Consumed by both `html.rs` (merged
into the JSON payload) and `svg.rs` (used directly for coordinate generation).

```rust
pub struct LayoutResult {
    pub positions: HashMap<String, Vec2>,   // node id → (x, y)
    pub bounds:    Rect,                    // bounding box of all positions
}

pub struct Vec2 {
    pub x: f64,
    pub y: f64,
}
```

`Rect` is `kurbo::Rect` in the SVG path; for the HTML path it is serialized as
`{"x": f64, "y": f64, "w": f64, "h": f64}` in the JSON payload.

---

## RenderError

```rust
pub enum RenderError {
    ParseError(String),     // tree-sitter parse failure with message
    EmptyGraph,             // gram file parsed successfully but contains no nodes
    LayoutError(String),    // layout failed (e.g., disconnected graph with no edges)
}

impl std::fmt::Display for RenderError { … }
impl std::error::Error for RenderError {}
```

---

## JSON data schema (HTML path)

`GramGraph` serializes to the following JSON, embedded as
`<script type="application/json" id="gram-data">` in the HTML template.

```json
{
  "nodes": [
    {
      "id": "n1",
      "labels": ["Person"],
      "properties": {"name": "Alice"},
      "is_nested": false
    }
  ],
  "edges": [
    {
      "id": "e1",
      "source": "n1",
      "target": "n2",
      "label": "KNOWS",
      "properties": {},
      "directed": true
    }
  ],
  "paths": [
    {
      "id": "p1",
      "members": ["n1", "e1", "n2"]
    }
  ],
  "nested": [
    {
      "parent": "n3",
      "children": ["n4", "n5"]
    }
  ],
  "layout": {
    "positions": {"n1": {"x": 100.0, "y": 200.0}},
    "bounds": {"x": 0.0, "y": 0.0, "w": 400.0, "h": 400.0}
  }
}
```

Note: the layout is pre-computed by Rust (spring embedder) and embedded in the
JSON so that the browser-side code only does rendering, not physics. This
keeps the HTML rendering code simple and the output deterministic.

---

## State transitions

```
.gram file
  ──parse_gram()──► GramGraph
                       │
              ┌────────┴────────┐
              ▼                 ▼
        layout::compute()   (layout for nested children, recursive)
              │
              ▼
        LayoutResult
              │
         ┌───┴───┐
         ▼       ▼
      html.rs  svg.rs
         │       │
         ▼       ▼
      String  String
    (HTML)    (SVG)
```
