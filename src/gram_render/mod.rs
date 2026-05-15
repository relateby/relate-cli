//! Gram visualization library (SC-006: zero CLI dependencies).
//!
//! This module and its submodules (`graph`, `layout`, `html`, `svg`) intentionally import
//! no CLI crates (`clap`, `tokio`, `rmcp`, `anyhow`). All types are plain Rust with optional
//! `serde` derives. This boundary is enforced by convention — verify with:
//!
//! ```text
//! grep -r "use clap\|use tokio\|use rmcp\|use anyhow" src/gram_render/
//! ```
//! which should produce no output.
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
