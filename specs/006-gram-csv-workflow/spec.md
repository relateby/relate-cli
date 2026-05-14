# Feature Specification: Gram CSV Workflow

**Feature Branch**: `006-gram-csv-workflow`
**Created**: 2026-05-12
**Status**: Draft — Design Phase Required (see Open Design Questions)
**Input**: User description: "gram workflow: 2 CSV files (node + relationship), apply to a cypher statement, creating a Neo4j graph, then read entire graph using cypher, producing a .gram file, which can be sent to another database to recreate the graph"

## Clarifications

### Session 2026-05-12

- Q: How many Cypher statements are required for the load step? → A: Each CSV file requires its own separate Cypher statement — one for creating nodes, one for creating relationships. Nodes are always created before relationships.
- Q: What is the conflict behavior when applying a `.gram` file to a database with existing data? → A: Conflict handling for CSV loading is already determined by the user's Cypher statement (MERGE vs CREATE). The same principle applies to `.gram` apply — but because Neo4j does not support nodes and relationships as query parameters, the existing `query --apply` command cannot be reused for `.gram` application. A dedicated design phase is required before this can be specified further.

## Open Design Questions *(design phase required before planning)*

The following questions must be resolved before this feature can proceed to implementation planning. They represent architectural decisions with significant impact on command design, data modeling, and user experience.

### DQ-001: Identity Mapping Between Neo4j and .gram

**Resolved by RFC-004** — see §Identity Mapping. Short answer: `--id-property` names a user-defined property used as the gram identifier on both sides; internal Neo4j IDs are never used.

Neo4j assigns internal element identifiers that are not stable across databases or exports. The `.gram` format uses its own node identifier syntax. It is not yet defined how these two identity systems map to each other during export (Neo4j → `.gram`) or import (`.gram` → Neo4j).

**Questions to resolve**:
- When exporting a graph to `.gram`, what becomes the `.gram` node identifier — a Neo4j internal ID, a user-defined property (e.g., a `uuid` field), or something generated at export time?
- When importing a `.gram` file into Neo4j, how are `.gram` identifiers used — as a lookup key for MERGE, as a property on the created node, or discarded?
- How should identifier conflicts be handled if the same `.gram` identifier appears in both the file and the target database?

### DQ-002: Dedicated Commands for Gram Send/Receive

**Resolved by RFC-004** — see §Command Design. Short answer: two new top-level commands `relate put <file.gram>` (gram → Neo4j, MERGE/upsert) and `relate get [--graph <name>]` (Neo4j → gram). The `query` command gains `--format gram|json|csv` for ad-hoc serialization.

The existing `query --apply` command reads parameterized Cypher + CSV rows and executes one statement per row. This approach cannot be adapted to `.gram` because Neo4j Cypher has no support for nodes and relationships as query parameters.

**Questions to resolve**:
- Are dedicated subcommands needed (e.g., `relate write --gram` / `relate read --gram`), or should `.gram` support be integrated into existing commands as a format flag?
- What is the invocation model for applying a `.gram` file — does the user provide a template Cypher statement (as with CSV), or does the tool generate the necessary Cypher internally from the `.gram` structure?
- Should send (`.gram` → Neo4j) and receive (Neo4j → `.gram`) be symmetric operations under the same command, or separate commands?

### DQ-003: .gram Format Compatibility with Neo4j

**Resolved by RFC-004** — see §Neo4j Type Mapping and §Well-Known Tagged Strings. Short answer: basic property graphs only (annotations and generic patterns cause a descriptive error on import); Neo4j spatial/temporal types map to well-known tagged strings (e.g., `` date-time`…` ``, `` point`SRID=…` ``).

The `.gram` format supports a broader set of data types and structural concepts than Neo4j natively represents, including annotations and generic pattern notation. Not all valid `.gram` files are representable as Neo4j graphs, and not all Neo4j graphs can be losslessly round-tripped through `.gram`.

**Questions to resolve**:
- Which `.gram` features are in scope for Neo4j interop: basic property graphs only, or also annotations and generic patterns?
- When exporting a Neo4j graph to `.gram`, are there Neo4j-specific types (e.g., spatial, temporal) that need explicit mapping to `.gram` equivalents?
- Should the tool warn or fail when encountering `.gram` structures it cannot map to Neo4j?

## User Scenarios & Testing *(mandatory)*

### User Story 1 - Load Graph from CSV Files (Priority: P1)

A user has two CSV files — one describing nodes and one describing relationships — and two separate Cypher statements, one per file, that define how to load each. They run a load command that first applies the node Cypher statement to every row in the nodes CSV, then applies the relationship Cypher statement to every row in the relationships CSV, creating a populated graph in a connected Neo4j database.

**Why this priority**: This is the entry point of the workflow. Without it, no graph exists to export. It delivers standalone value: graph population from structured tabular data.

**Independent Test**: Can be tested by providing sample node and relationship CSV files each with their own parameterized Cypher statement and verifying the graph is created in the target database.

**Acceptance Scenarios**:

1. **Given** a nodes CSV, a node-creation Cypher statement, a relationships CSV, and a relationship-creation Cypher statement, **When** the user runs the load command, **Then** nodes and relationships are created in the Neo4j graph matching the CSV contents.
2. **Given** a nodes CSV with a header row, **When** the node Cypher statement is applied, **Then** column names are used as parameter names bound into the statement.
3. **Given** a relationships CSV referencing node identifiers that exist, **When** the relationship Cypher statement is applied, **Then** relationships are created between the correct nodes.
4. **Given** both CSVs and their respective Cypher statements, **When** the load runs, **Then** all node rows are processed before any relationship rows.
5. **Given** a CSV file with a malformed row, **When** the command runs, **Then** an informative error identifies the bad row and file, and the command exits with a non-zero code.
6. **Given** the database is unreachable, **When** the command runs, **Then** a clear connection error is reported and no partial data is written.

---

### User Story 2 - Export Graph to .gram File (Priority: P2)

After the graph is populated, the user reads the entire graph back and exports it as a `.gram` file — a portable, human-readable representation of the graph. The exact command form depends on resolution of DQ-002 and DQ-003.

**Why this priority**: Depends on P1 (a graph must exist), but delivers the portability value: the graph can now be shared or archived.

**Independent Test**: Can be tested independently against any existing Neo4j graph by running the read/export command and verifying the output is a valid `.gram` file.

**Acceptance Scenarios**:

1. **Given** a populated Neo4j graph, **When** the user runs the export command, **Then** the output is a `.gram` file containing all nodes and relationships.
2. **Given** a graph with node properties and relationship properties, **When** exported, **Then** all properties appear correctly in the `.gram` output.
3. **Given** the user specifies an output file path, **When** the command runs, **Then** the `.gram` file is written to that path.
4. **Given** no output path is specified, **When** the command runs, **Then** the `.gram` content is written to stdout.

---

### User Story 3 - Recreate Graph in Another Database (Priority: P3)

A user takes a `.gram` file produced in Story 2 and applies it to a second Neo4j database, recreating the graph structure and data there. The exact mechanism depends on resolution of DQ-001 and DQ-002.

**Why this priority**: Completes the end-to-end portability scenario. Independently testable using any valid `.gram` file against an empty database.

**Independent Test**: Can be tested by applying any valid `.gram` file to an empty database and verifying the resulting graph matches the source.

**Acceptance Scenarios**:

1. **Given** a `.gram` file and a target Neo4j database, **When** the user applies the file, **Then** the target database contains nodes and relationships matching the `.gram` contents.
2. **Given** a `.gram` file with relationship entries, **When** applied, **Then** relationships reference the correct nodes in the target database.
3. **Given** an empty target database, **When** the full workflow runs end-to-end (CSV → graph → `.gram` → new database), **Then** the resulting graph in the second database is structurally equivalent to the first.

---

### Edge Cases

- What happens when a node CSV row is missing a value required by the Cypher statement?
- What happens when a relationship CSV references a node identifier not present in the nodes CSV?
- What happens when the `.gram` file contains identifiers that already exist in the target database? *(depends on DQ-001)*
- What happens when one of the CSV files is empty?
- How does the system handle CSV files with thousands of rows?
- What happens when a `.gram` file contains annotations or generic patterns not representable in Neo4j? *(depends on DQ-003)*

## Requirements *(mandatory)*

### Functional Requirements

- **FR-001**: Users MUST be able to specify two CSV files (nodes and relationships) as input to a graph load operation.
- **FR-002**: Users MUST be able to provide two separate Cypher statements — one applied per row of the nodes CSV to create nodes, and one applied per row of the relationships CSV to create relationships. CSV column header values are bound as named parameters in each statement.
- **FR-003**: The load operation MUST create nodes and relationships in a connected Neo4j database.
- **FR-004**: The load operation MUST process all node rows before any relationship rows to satisfy referential integrity.
- **FR-005**: Users MUST be able to export a Neo4j graph to a `.gram` file. *(command form subject to DQ-002)*
- **FR-006**: The exported `.gram` file MUST preserve node labels, relationship types, and all properties from the source graph. *(type mapping subject to DQ-003)*
- **FR-007**: Users MUST be able to apply a `.gram` file to a Neo4j database to recreate the described graph. *(mechanism subject to DQ-001 and DQ-002)*
- **FR-008**: All commands MUST report errors with enough detail for the user to identify the source (bad CSV row, connection failure, malformed input, etc.).
- **FR-009**: All commands MUST accept Neo4j connection details via flags or environment variables; credentials MUST NOT be hardcoded or prompted interactively.

### Key Entities

- **Node CSV**: Tabular file where each row describes one node; column headers map to Cypher parameter names in the node-creation statement.
- **Relationship CSV**: Tabular file where each row describes one relationship; column headers map to Cypher parameter names in the relationship-creation statement, including columns that identify the source and target nodes.
- **Node Cypher Statement**: A parameterized query applied once per node CSV row to create nodes; receives all CSV column values as named parameters.
- **Relationship Cypher Statement**: A parameterized query applied once per relationship CSV row to create relationships; receives all CSV column values as named parameters.
- **Graph**: The set of nodes and relationships created in or read from a Neo4j database.
- **Gram File**: A `.gram` formatted file encoding graph structure and properties in a portable, human-readable text format. Supports a broader set of types and structures (annotations, generic pattern notation) than Neo4j's native property graph model.

## Success Criteria *(mandatory)*

### Measurable Outcomes

- **SC-001**: A user can complete the full workflow — load from two CSVs with two Cypher statements, export to `.gram`, apply to a second database — in a single scripted sequence without manual intervention or interactive prompts.
- **SC-002**: The exported `.gram` file, when applied to an empty database, produces a graph structurally identical to the source (same node count, relationship count, labels, types, and properties).
- **SC-003**: Error messages identify the specific file, row number, or graph element responsible for every failure.
- **SC-004**: The load step completes a 10,000-row CSV against a local database in under 60 seconds.

## Assumptions

- Users have an accessible Neo4j instance for both the load and recreate steps (may be the same or different instances).
- The node CSV and relationship CSV share a common identifier column; the relationship Cypher statement uses that identifier to match endpoint nodes.
- Both Cypher statements provided by the user are valid and use CSV column header names as parameter placeholders.
- The `.gram` format is the existing format already supported by `relate` (via tree-sitter-gram).
- The read operation defaults to retrieving all nodes and relationships unless the user provides a scoping Cypher query.
- Advanced batching configuration is out of scope; sensible defaults are used for large loads.
- Conflict handling during CSV load is entirely controlled by the user's Cypher statement (MERGE vs CREATE); the tool does not impose a default merge strategy.
