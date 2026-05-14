# Implementation Plan: Gram Visualization (render subcommand)

**Branch**: `008-gram-render` | **Date**: 2026-05-14 | **Spec**: [spec.md](spec.md)  
**RFC**: [RFC-006-render-command.md](../../proposals/RFC-006-render-command.md)

## Summary

`relate render` visualizes `.gram` files as either a standalone interactive HTML
page (Paper.js + d3-force, no external deps) or a static deterministic SVG (pure
Rust: kurbo geometry + custom spring embedder). The rendering logic is isolated
behind a clean library API in `src/gram_render/` so it can be extracted to a
standalone crate without refactoring call sites. `src/commands/render.rs` is a
thin dispatch layer: parse args, read file, call library, write output.

The constitution requires three additions beyond the RFC scope: a `--json` flag
(outputs `{"output":"…","format":"…"}`), an MCP tool wrapping `render`, and a
`skills/relate-render/SKILL.md` agent discovery file.

## Technical Context

**Language/Version**: Rust (stable, ≥ 1.80; MSRV tracks relate-cli's existing MSRV)  
**Primary Dependencies**:
- `tree-sitter-gram 0.3.7` — already in use; gram parsing
- `kurbo 0.11` — 2D curves, Bézier paths, bounding boxes (SVG path)
- `svg 0.17` — SVG element builder and serializer (SVG path)
- `geo 0.28` (features: `convex_hull`) — convex hull over point sets (SVG path)
- `serde_json 1` — already present; GramGraph → JSON for HTML template
- `open` crate — cross-platform file open for `--open` flag
- Paper.js core (~220 KB), d3-force (~45 KB) — bundled JS assets, not Rust crates

**Storage**: Filesystem only (read .gram, write .html or .svg)  
**Testing**: `cargo test` (unit tests for parser, layout, SVG geometry; integration tests for golden-file SVG output)  
**Target Platform**: macOS, Linux, Windows (same as relate-cli)  
**Project Type**: CLI subcommand + incubated library module (`src/gram_render/`)  
**Performance Goals**: Render completes in < 5 seconds for ≤ 200 nodes on a standard developer laptop  
**Constraints**:
- Zero external runtime dependencies (no Node.js, no system binaries, no CDN)
- HTML output must open from `file://` URI without a web server
- SVG output must be byte-for-byte identical across runs for the same input
- `src/gram_render/` must compile with no dependency on `clap`, `tokio`, or any CLI crate

**Scale/Scope**: Single gram file per invocation; typical files have < 100 nodes

## Constitution Check

*GATE: Must pass before Phase 0 research. Re-checked after Phase 1 design.*

| Principle | Status | Notes |
|-----------|--------|-------|
| I. CLI-Friendly | ✅ Pass | `relate render <file>` writes output file; errors to stderr; exit code 1 on failure. Stdin skipped (render requires a named output file; acceptable deviation for visual output). |
| II. Human-Readable | ✅ Pass | Output is a visual diagram. Render errors include the offending line/token from the gram file where possible. |
| III. Agent-Friendly | ⚠️ Scope addition | RFC omits `--json`, MCP tool, and SKILL.md — all required by constitution. Added to implementation scope below. |
| IV. Self-Contained Help | ✅ Pass | clap derive; `--help` includes purpose, all flags with defaults, two examples. |

**Agent-Friendly additions (Principle III):**

1. **`--json` flag**: prints `{"output":"<path>","format":"html|svg"}` to stdout on success, `{"error":"<message>"}` on failure. Silent otherwise.
2. **MCP tool `render_gram`**: wraps `gram_render::render_svg()`; accepts gram source string, returns SVG string. Registered in `src/commands/mcp.rs`.
3. **`skills/relate-render/SKILL.md`**: agent discovery file; numbered workflow, checkpoints, exit criteria, anti-rationalization table. `skills/relate/SKILL.md` routing table updated.

No constitution violations. No Complexity Tracking needed.

## Project Structure

### Documentation (this feature)

```text
specs/008-gram-render/
├── plan.md              # This file
├── research.md          # Phase 0 output
├── data-model.md        # Phase 1 output
├── quickstart.md        # Phase 1 output
├── contracts/
│   ├── relate-render.md        # CLI contract
│   └── gram-render-lib.md      # Rust library API contract
└── tasks.md             # Phase 2 output (/speckit-tasks)
```

### Source Code

```text
src/
├── cli.rs                  # add RenderArgs, Format enum, --json flag
├── commands/
│   ├── render.rs           # thin dispatch: args → gram_render → file write → open
│   └── mcp.rs              # add render_gram MCP tool
└── gram_render/            # incubated library — zero CLI dependencies
    ├── mod.rs              # pub API: parse_gram(), render_html(), render_svg()
    ├── graph.rs            # GramGraph + tree-sitter traversal
    ├── layout.rs           # spring embedder → LayoutResult
    ├── html.rs             # GramGraph + LayoutResult → HTML string
    └── svg.rs              # GramGraph + LayoutResult → SVG string

assets/vendor/
├── paper-core.min.js       # Paper.js core bundle (include_str! at build time)
└── d3-force.min.js         # d3-force standalone bundle (include_str! at build time)

skills/
├── relate/SKILL.md         # updated routing table
└── relate-render/SKILL.md  # new agent discovery file

tests/
├── render_html.rs          # integration: gram file → HTML (smoke test)
└── render_svg/
    ├── mod.rs              # golden-file SVG comparison
    └── fixtures/           # *.gram input files + *.svg expected output
```
