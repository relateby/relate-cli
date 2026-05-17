# Tasks: Gram Visualization (render subcommand)

**Branch**: `008-gram-render`  
**Spec**: [spec.md](spec.md) | **Plan**: [plan.md](plan.md)  
**RFC**: [RFC-006-render-command.md](../../proposals/RFC-006-render-command.md)

## Format: `[ID] [P?] [Story?] Description`

- **[P]**: Can run in parallel (different files, no shared dependencies)
- **[US#]**: Maps to user story from spec.md

---

## Phase 1: Setup

**Purpose**: Add dependencies and create the module skeleton so all phases can proceed.

- [x] T001 Add kurbo, svg, geo (convex_hull feature), and open crate dependencies to Cargo.toml
- [x] T002 [P] Create assets/vendor/ directory and add paper-core.min.js and d3-force.min.js bundles; record SHA-256 checksums in assets/vendor/CHECKSUMS
- [x] T003 [P] Create src/gram_render/ module with stub implementations: mod.rs (re-exports), graph.rs (empty types), layout.rs (zero-position stub), html.rs (returns empty string), svg.rs (returns empty string)

---

## Phase 2: Foundational (Blocking Prerequisites)

**Purpose**: Core parsing, layout, and CLI wiring that all user stories depend on.

**⚠️ CRITICAL**: No user story work can begin until this phase is complete.

- [x] T004 Define all GramGraph data types with serde derives in src/gram_render/graph.rs: GramGraph, GramNode, GramEdge, GramPath, PathMember, NestingRelation, LayoutResult, Vec2, RenderError
- [x] T005 Implement parse_gram() tree-sitter traversal in src/gram_render/graph.rs: walk node_pattern, relationship_pattern, path_pattern, nested_graph node kinds to populate GramGraph; return RenderError::EmptyGraph when no nodes found
- [x] T006 [P] Implement layout::compute() spring embedder in src/gram_render/layout.rs: circular initial placement, 500 iterations of Coulomb repulsion + Hooke attraction + centering force with linear cooling, normalize to 600×600 canvas; recursive call for nested children
- [x] T007 [P] Add RenderArgs (file, --format, --output, --open, --json), OutputFormat enum, and Commands::Render variant to src/cli.rs; wire dispatch in src/main.rs as synchronous call (no .await)
- [x] T008 Implement stub dispatch in src/commands/render.rs: read file, call gram_render::parse_gram(), derive default output path as <input-stem>.<format>, call render_html()/render_svg() stubs, write output file; return exit code 1 on RenderError, 2 on I/O error

**Checkpoint**: `cargo build` succeeds. `relate render --help` shows all flags. `relate render any.gram` runs without panic (produces empty output file from stub).

---

## Phase 3: User Story 1 — Interactive HTML Browser Visualization (Priority: P1) 🎯 MVP

**Goal**: `relate render graph.gram` produces a self-contained .html file with pan/zoom, click-to-inspect, and colored path envelopes. Opens from file:// without a server.

**Independent Test**: Run `relate render sample.gram`, open the .html file in a browser (offline), and verify: graph is visible, named paths have distinct colored envelopes, clicking a node shows its properties in the sidebar.

- [x] T009 [P] [US1] Write assets/templates/render.js: parse gram-data JSON; run d3-force to convergence synchronously; draw path envelopes (convex hull of member node positions → expanded → Bézier-rounded Paper.js Path with distinct per-path HSL fill); draw edges (directed: Line + triangular Path arrowhead; undirected: plain Line); draw nodes (Group: Path.Circle + PointText label); draw compound outlines (Rectangle from Group.bounds + padding); wire pan/zoom view event handlers; wire project.hitTest() click-to-inspect → sidebar pre element
- [x] T010 [P] [US1] Implement gram_render::render_html() in src/gram_render/html.rs: derive LayoutResult, serialize GramGraph+layout to JSON via serde_json, embed PAPER_JS (include_str! assets/vendor/paper-core.min.js), D3_FORCE (include_str! assets/vendor/d3-force.min.js), data JSON, and render.js (include_str! assets/templates/render.js) into a complete HTML5 document string
- [x] T011 [US1] Wire Format::Html branch in src/commands/render.rs to call gram_render::render_html() and write output; run end-to-end with a sample .gram file and verify the HTML opens correctly in a browser

**Checkpoint**: `relate render sample.gram` writes a .html file that opens offline, shows the graph with interactive path envelopes, and click-to-inspect works.

---

## Phase 4: User Story 2 — Static SVG for Documentation (Priority: P2)

**Goal**: `relate render graph.gram --format svg` produces a deterministic, self-contained SVG with path envelopes, directed/undirected edges, and compound node outlines.

**Independent Test**: Run `relate render sample.gram --format svg` twice; diff the two files — they must be identical. Embed the SVG in a Markdown README and verify it renders on GitHub.

- [x] T012 [P] [US2] Implement path envelope drawing in src/gram_render/svg.rs: collect member node positions into geo::MultiPoint, call ConvexHull::convex_hull(), expand vertices radially from centroid, connect with kurbo::CubicBez (tangent-continuous), serialize to svg::node::element::Path with per-path HSL fill and semi-transparent opacity
- [x] T013 [P] [US2] Implement edge drawing in src/gram_render/svg.rs: directed edges as kurbo::Line with a svg marker arrowhead triangle defined in <defs>; undirected edges as plain svg line; edge label as svg Text at midpoint
- [x] T014 [P] [US2] Implement node and compound outline drawing in src/gram_render/svg.rs: regular nodes as svg circle with label Text below; compound nodes (NestingRelation parents) as kurbo::RoundedRect from union of children Rects with padding; nested children laid out within parent bounds
- [x] T015 [US2] Assemble render_svg() in src/gram_render/svg.rs: create svg::Document with viewBox, add layered <g> groups (path-envelopes → edges → nodes → compound-outlines → labels), return complete SVG string; depends on T012–T014
- [x] T016 [US2] Add golden-file SVG tests in tests/render_svg/: create fixture .gram files (simple graph, graph with named paths, graph with nested structure); generate expected .svg files; assert render_svg() output matches fixtures byte-for-byte on re-run

**Checkpoint**: `relate render sample.gram --format svg` writes a valid SVG. Running twice produces identical output. The SVG embeds in a Markdown README and renders correctly.

---

## Phase 5: User Story 3 — Auto-Open After Rendering (Priority: P3)

**Goal**: `--open` flag opens the output file in the system default viewer immediately after a successful render.

**Independent Test**: Run `relate render sample.gram --open`; verify the file is written and the system's default browser (or SVG viewer) opens it automatically.

- [x] T017 [US3] Implement --open flag in src/commands/render.rs: after successful file write, call open::that(&output_path)?; only fires on success; failure to open is logged to stderr but does not change exit code; compatible with --json flag

**Checkpoint**: `relate render sample.gram --open` opens the browser automatically. Render failure suppresses the open call.

---

## Phase 6: User Story 4 — Controlled Output Path & Machine-Readable Output (Priority: P4)

**Goal**: `--output` flag writes to an explicit path; `--json` prints structured output for scripting.

**Independent Test**: Run `relate render sample.gram --output /tmp/out.html`; verify file appears at /tmp/out.html. Run with `--json`; verify stdout is valid JSON with an `output` key.

- [x] T018 [US4] Implement --output flag and format-precedence edge case in src/commands/render.rs: when --output is provided, use it as-is; when absent, derive <input-stem>.<format>; format flag always determines content regardless of output file extension; overwrite silently if output exists
- [x] T019 [US4] Implement --json flag in src/commands/render.rs: on success print {"output":"<path>","format":"html|svg"} to stdout; on RenderError or I/O error print {"error":"<message>"} to stdout; no other stdout output when --json is active; stderr output suppressed when --json is active

**Checkpoint**: `relate render s.gram --output /tmp/x.html --json` prints `{"output":"/tmp/x.html","format":"html"}` to stdout. Script `$(relate render s.gram --json | jq -r .output)` returns the output path.

---

## Phase 7: Polish & Constitution Compliance

**Purpose**: Agent-friendliness (Principle III), self-contained help (Principle IV), and library isolation verification (FR-013 / SC-006).

- [x] T020 [P] Register render_gram MCP tool in src/commands/mcp.rs: input schema {gram_source: string, format: "svg"|"html" (default "svg")}; call gram_render::parse_gram() then render_svg()/render_html(); return content array with rendered string; return isError on RenderError
- [x] T021 [P] Create skills/relate-render/SKILL.md: YAML frontmatter (name: relate-render, description, triggers); numbered workflow (1. identify gram file, 2. run relate render, 3. verify output, 4. open or embed); checkpoints after each step; exit criteria (output file written and openable); anti-rationalization table
- [x] T022 Update skills/relate/SKILL.md routing table to add relate-render entry with command, description, and trigger keywords
- [x] T023 [P] Add comprehensive --help text to RenderArgs in src/cli.rs: about string with purpose; per-flag help strings including defaults; two concrete usage examples in the doc comment
- [x] T024 [P] Verify SC-006 library isolation in src/gram_render/mod.rs: add a module-level doc comment asserting no CLI crate dependencies; confirm with `cargo check --package` that gram_render compiles when clap/tokio are absent from its direct imports; document the verification step in plan.md

---

## Dependencies & Execution Order

### Phase Dependencies

- **Setup (Phase 1)**: No dependencies — start immediately; T002 and T003 are parallel
- **Foundational (Phase 2)**: Depends on Phase 1 — T004→T005→T006 sequential; T006 and T007 parallel after T004
- **US1 (Phase 3)**: Depends on Phase 2 — T009 and T010 parallel; T011 depends on T009+T010
- **US2 (Phase 4)**: Depends on Phase 2 — T012, T013, T014 parallel; T015 depends on all three; T016 depends on T015
- **US3 (Phase 5)**: Depends on Phase 3 (needs working render output to open)
- **US4 (Phase 6)**: Depends on Phase 2; T018 and T019 are parallel
- **Polish (Phase 7)**: Depends on Phase 2; T020 depends on Phase 4 (SVG available); T021–T024 parallel

### User Story Dependencies

- **US1 (P1)**: Depends on Foundational only — no dependency on US2/US3/US4
- **US2 (P2)**: Depends on Foundational only — no dependency on US1/US3/US4
- **US3 (P3)**: Depends on US1 completing (needs a working HTML output to open)
- **US4 (P4)**: Depends on Foundational only — output path logic is in commands/render.rs

### Within Each Phase

- All [P]-marked tasks in the same phase can run concurrently
- T005 (parse_gram) must complete before T006 (layout) can use GramGraph type
- T009 and T010 (HTML phase) are parallel; T011 depends on both
- T012, T013, T014 (SVG geometry) are parallel; T015 assembles them

---

## Parallel Execution Examples

### Phase 2 (Foundational) — after T004 completes:

```
T005: implement parse_gram() traversal in graph.rs
T006: implement layout::compute() spring embedder in layout.rs   ← parallel with T005
T007: add RenderArgs + Commands::Render to cli.rs + main.rs      ← parallel with T005
```

### Phase 3 (US1 — HTML):

```
T009: write assets/templates/render.js (Paper.js rendering)
T010: implement render_html() in src/gram_render/html.rs         ← parallel with T009
```

### Phase 4 (US2 — SVG geometry):

```
T012: path envelope geometry in svg.rs
T013: edge drawing in svg.rs                                      ← parallel with T012
T014: node and compound outline drawing in svg.rs                 ← parallel with T012, T013
```

### Phase 7 (Polish):

```
T020: register render_gram MCP tool in mcp.rs
T021: create skills/relate-render/SKILL.md                        ← parallel with T020
T023: add --help text to RenderArgs in cli.rs                     ← parallel with T020, T021
T024: verify SC-006 library isolation                             ← parallel with T020, T021, T023
```

---

## Implementation Strategy

### MVP (User Story 1 only)

1. Complete Phase 1 (Setup)
2. Complete Phase 2 (Foundational) — **blocks everything**
3. Complete Phase 3 (US1: HTML output)
4. **STOP and VALIDATE**: open the HTML file offline, verify path envelopes and click-to-inspect
5. Ship the MVP — `relate render graph.gram` works end-to-end

### Incremental Delivery

1. MVP (above) → users can explore gram files interactively
2. Add US2 (Phase 4) → static SVG for documentation and CI
3. Add US3 (Phase 5) → `--open` convenience
4. Add US4 (Phase 6) → pipeline integration via `--output` and `--json`
5. Add Phase 7 (Polish) → agent-friendly, constitution-compliant

### Notes

- The rendering library (src/gram_render/) has zero CLI dependencies by construction — verify this at T024 before shipping
- Golden-file SVG tests (T016) are the primary regression guard for the SVG path; run them on every CI push
- T002 (vendoring JS assets) requires downloading paper-core.min.js and d3-force.min.js; record exact versions and checksums in assets/vendor/CHECKSUMS
- Render is a synchronous command — do not introduce async into src/gram_render/ or src/commands/render.rs
