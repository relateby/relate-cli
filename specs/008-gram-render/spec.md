# Feature Specification: Gram Visualization (render subcommand)

**Feature Branch**: `008-gram-render`  
**Created**: 2026-05-14  
**Status**: Draft  
**RFC**: proposals/RFC-006-render-command.md

## User Scenarios & Testing *(mandatory)*

### User Story 1 — Explore a gram file visually in a browser (Priority: P1)

A developer has a `.gram` file describing a graph and wants to understand its
structure visually. They run a single command and get a standalone HTML file they
can open in any web browser — no server, no internet connection, no extra tools
required. The visualization lets them pan and zoom to explore the graph, and click
on nodes or edges to inspect their properties.

**Why this priority**: This is the primary use case. Most gram authors work
interactively, and a browser-based view is the lowest-friction way to get visual
feedback on what a gram file describes.

**Independent Test**: Can be fully tested by running `relate render sample.gram`,
opening the output `.html` file in a browser, and verifying the graph is visible
and interactive.

**Acceptance Scenarios**:

1. **Given** a valid `.gram` file, **When** `relate render` is run with no format
   flag, **Then** a `.html` file is written next to the input file and the
   developer can open it in a browser without any additional setup.

2. **Given** a gram file with multiple named paths, **When** the HTML is opened,
   **Then** each path is visually distinguished from the others and from
   background graph topology (e.g., distinct color or enclosing shape per path).

3. **Given** an HTML output file, **When** the developer clicks a node or edge,
   **Then** the labels and properties of that element are displayed.

4. **Given** an HTML output file, **When** the developer is offline, **Then** the
   visualization still loads and renders correctly (no CDN or network dependency).

---

### User Story 2 — Generate a static SVG for documentation (Priority: P2)

A developer building documentation or a README wants to include a diagram of their
gram data model as a static image. They run `relate render --format svg` and get
a clean vector file they can embed directly in Markdown, HTML, or PDF output — no
browser required to produce it, and no external tools needed to render it.

**Why this priority**: Static SVG output supports documentation-as-code workflows
and CI pipelines where browser rendering is not available. It is the non-interactive
complement to the HTML output.

**Independent Test**: Can be fully tested by running
`relate render sample.gram --format svg`, opening the `.svg` file in a browser or
vector editor, and verifying graph structure is present and correct.

**Acceptance Scenarios**:

1. **Given** a valid `.gram` file, **When** `relate render --format svg` is run,
   **Then** a `.svg` file is written and contains a complete visual representation
   of the graph.

2. **Given** an SVG output file, **When** embedded in a Markdown document or HTML
   page, **Then** it scales correctly to fit its container without quality loss.

3. **Given** the same gram file, **When** `relate render --format svg` is run
   twice, **Then** both output files are identical (deterministic output).

4. **Given** a gram file with nested subgraph structures, **When** the SVG is
   produced, **Then** compound nodes are visually enclosed with their contained
   elements shown inside.

---

### User Story 3 — Open visualization immediately after rendering (Priority: P3)

A developer wants a quick visual without managing output file paths. They run
`relate render --open` and the visualization opens in their default browser
automatically after being written to disk.

**Why this priority**: Reduces friction for interactive use; `--open` is a
conventional CLI affordance for this pattern.

**Independent Test**: Can be fully tested by running `relate render sample.gram --open`
and verifying the browser opens with the diagram automatically.

**Acceptance Scenarios**:

1. **Given** `--open` is passed, **When** rendering completes, **Then** the output
   file opens in the system default browser (or SVG viewer for `--format svg`).

2. **Given** `--open` is passed and rendering fails, **Then** no browser is opened
   and the error is reported instead.

---

### User Story 4 — Control output file path (Priority: P4)

A developer integrating `relate render` into a build pipeline wants to specify
exactly where the output file is written, independent of the input file's location.

**Why this priority**: Build system integration requires predictable output paths;
the default naming convention is not always appropriate.

**Independent Test**: Can be tested by running
`relate render sample.gram --output docs/diagram.html` and verifying the file
appears at the specified path.

**Acceptance Scenarios**:

1. **Given** `--output <path>` is passed, **When** rendering completes, **Then** the
   output is written to exactly that path.

2. **Given** `--output` is not passed, **When** rendering completes, **Then** the
   output is written to `<input-stem>.<format>` alongside the input file.

---

### Edge Cases

- What happens when the gram file is empty or contains no nodes?
- How does the visualization handle a gram file where all elements belong to a
  single named path vs. no named paths at all?
- What happens when a gram file describes very large graphs (hundreds of nodes)?
  Layout may become cluttered — the tool should warn but still produce output.
- What happens when two paths share all the same nodes (fully overlapping)?
- What happens when `--format svg` is specified but `--output` has a `.html`
  extension, or vice versa? The format flag takes precedence.
- What happens when the output path is not writable? A clear error must be shown.

## Requirements *(mandatory)*

### Functional Requirements

- **FR-001**: The tool MUST accept a `.gram` file path as its primary argument and
  produce a visual output file from it.
- **FR-002**: The tool MUST support an HTML output format that is fully
  self-contained (no external resources, no web server, no internet access required
  to open and view it).
- **FR-003**: The tool MUST support an SVG output format produced entirely without
  a browser or JS runtime, suitable for embedding in documentation.
- **FR-004**: HTML output MUST support pan and zoom navigation.
- **FR-005**: HTML output MUST support clicking a node or edge to view its labels
  and properties.
- **FR-006**: Both output formats MUST visually distinguish named gram paths from
  each other and from unlabelled graph topology.
- **FR-007**: Both output formats MUST visually distinguish directed edges from
  undirected edges.
- **FR-008**: Both output formats MUST render nested/compound graph structures so
  that a node containing a subgraph shows its contained elements visually enclosed
  within it.
- **FR-009**: SVG output MUST be deterministic: the same gram file always produces
  the same SVG.
- **FR-010**: The default output filename MUST be derived from the input filename
  with the appropriate extension (`.html` or `.svg`).
- **FR-011**: The `--output` flag MUST allow overriding the output path.
- **FR-012**: The `--open` flag MUST open the output file in the system default
  viewer after writing it.
- **FR-013**: The rendering logic MUST be isolated behind a library interface with
  no dependency on CLI infrastructure, so it can be extracted as an independent
  library without refactoring the call sites.

### Key Entities

- **GramGraph**: The parsed intermediate representation of a gram file, capturing
  nodes (with labels and properties), edges (directed or undirected, with label and
  properties), named paths (ordered sequences of nodes and edges), and nesting
  relations (a node whose value is a subgraph).
- **Visual output**: A self-contained file (HTML or SVG) representing the graph
  structure, intended to be opened in a browser or embedded in documentation.
- **Named path**: A first-class gram construct — a labelled sequence of alternating
  nodes and edges — that requires distinct visual treatment separate from the
  underlying node/edge topology.

## Success Criteria *(mandatory)*

### Measurable Outcomes

- **SC-001**: A developer can go from a `.gram` file to a viewable diagram in a
  single command with no additional installation steps.
- **SC-002**: The HTML output opens correctly in a browser without an internet
  connection or local web server.
- **SC-003**: The SVG output can be embedded directly in a Markdown README and
  renders correctly on GitHub.
- **SC-004**: Named paths in the visualization are distinguishable by a casual
  viewer without any legend or prior explanation.
- **SC-005**: Rendering completes in under five seconds for gram files with fewer
  than 200 nodes on a standard developer laptop.
- **SC-006**: The rendering library module has no compile-time dependency on any
  CLI-specific crate, verifiable by building it in isolation.

## Assumptions

- Users have a modern web browser available to view HTML output (Chrome, Firefox,
  Safari, or Edge).
- Gram files encountered in practice are small to medium in size (fewer than a few
  hundred nodes). Very large files are a deferred concern.
- The rendering library is incubated inside relate-cli and may be extracted to its
  own published crate once the API is stable; the spec does not require publication
  to be in scope.
- Annotation rendering (gram's post-declaration property assignment via `@` syntax)
  is out of scope until parser support for extracting annotations is added to
  relate-cli.
- The `--open` behaviour uses the system's default file association; no specific
  browser is targeted.
- The visual style (colors, fonts, node shapes) is implementation-defined and not
  user-configurable in the initial version.
