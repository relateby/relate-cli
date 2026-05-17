//! Gram visualization library (SC-006: zero CLI dependencies).
//!
//! This module and its submodules (`graph`, `layout`, `html`, `svg`) must not import
//! anything from `crate::cli` or `crate::commands` — they are pure library code with no
//! CLI dependency. All types are plain Rust with optional `serde` derives. Verify the
//! boundary with:
//!
//! ```text
//! grep -rn "crate::" src/gram_render/
//! ```
//!
//! which should produce no output (only `super::` paths within the module are allowed).
//!
//! Public API: `parse_gram()`, `render_html()`, `render_svg()`, and the `GramGraph` type family.

pub mod graph;
pub mod html;
pub mod layout;
pub mod svg;

#[allow(unused_imports)]
pub use graph::{
    parse_gram, GramEdge, GramGraph, GramNode, GramPath, NestingRelation, PathMember, RenderError,
};
pub use html::render_html;
pub use svg::render_svg;
