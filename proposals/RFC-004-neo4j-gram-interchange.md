---
number: "004"
title: "Gram Interchange — CSV, JSON, and Neo4j"
status: "Draft"
date: "2026-05-13"
authors:
  - "Andreas Kollegger <andreas.kollegger@neo4j.com>"
---

# RFC-004: Gram Interchange — CSV, JSON, and Neo4j

## Summary

This RFC defines how `.gram` interoperates with three data sources and targets:
CSV files, JSON/JSON Schema, and Neo4j. It establishes a shared foundation of
**well-known tagged strings** — a two-tier set of type tags built on JSON Schema
`format` names — that applies across all three interchange scenarios.

For **CSV ↔ gram**: CSV has no type system; type information comes from the
user-supplied Cypher statements in the loading workflow. No additional convention
is needed for gram-native types.

For **JSON ↔ gram**: gram's native types cover most of JSON's type system directly.
`null` has no gram native equivalent and is handled by omission. Gram-only types
(range, measurement, hexadecimal, octal) have no JSON equivalents. JSON Schema
`format` string subtypes map to well-known tagged strings.

For **Neo4j ↔ gram**: gram files carry a self-describing top-level record
(`{ kind: social, graph: "name" }`) that provides the canonical graph name, removing
reliance on filename conventions. Operations are exposed as `relate put` (gram →
Neo4j, with sub-graph membership tracking) and `relate get` (Neo4j → gram). Both are
designed as a GQL named-graph compatibility layer, ready to delegate to `CREATE
GRAPH` / `GRAPH <name> MATCH` when Neo4j implements ISO/IEC 39075:2024. The `query`
command gains `--format gram|json|csv` for ad-hoc result serialization. Neo4j spatial
and temporal types map to well-known tagged strings using ISO 8601 and Extended WKT.

The well-known tag set is proposed as an official convention in both `tree-sitter-gram`
and `pattern-rs` (`gram_codec`). `.gram`-only constructs (annotations, generic
patterns) are out of scope for v1.

## Architectural Boundary

Much of the work described in this RFC — well-known tag decoding, type mapping
between Neo4j and gram, gram file header parsing, sub-graph membership tracking —
belongs in libraries rather than in a CLI tool. The natural homes are `pattern-rs`
(gram value type system, well-known tags, file header conventions) and a future
`neo4j-gram` crate (Neo4j ↔ gram type mapping, MERGE Cypher generation).

`relate` implements these capabilities first because:
1. It is the first concrete consumer, and building against real use cases validates
   the API design before it is proposed upstream.
2. It provides fast feedback on which abstractions are load-bearing and which are
   over-engineered.

The implementation constraint is: **`relate` must treat this logic as a library
boundary from the start.** Concretely:

- Well-known tag decoding lives in a module with a stable, pure API — no CLI
  concerns, no I/O.
- Neo4j type conversion lives in a module that takes `neo4rs` value types and
  returns gram `Value` types (and vice versa), with no awareness of commands or flags.
- `put` / `get` / `query --format` are thin command handlers that call these modules.

When the logic is ready to externalize — either as a contribution to `pattern-rs`
or as a new crate — the CLI layer does not change; only the dependency path moves.
This is the same pattern the project already uses for `tree-sitter-cypher` and
`tree-sitter-gram`: the CLI calls the library, the library is independently useful.

## Motivation

`relate` needs to move graph data between CSV files, Neo4j databases, and `.gram`
files. Each transition involves a type system mismatch:

- **CSV** has no types — all values are strings. The user resolves this via their
  Cypher loading statements.
- **JSON** has a small fixed type set. `.gram` covers it almost completely with
  native types, but gram also has types (range, measurement) that JSON cannot
  represent, and JSON has `null` that gram cannot represent.
- **Neo4j** has rich spatial and temporal types. `.gram` has no built-in equivalents,
  but its `tagged_string` mechanism provides a portable serialization path.
- **`.gram` tagged strings** are open-ended by design, but without a shared convention
  every tool invents its own tag names. A well-known tag registry prevents fragmentation
  across `tree-sitter-gram`, `pattern-rs`, and tools like `relate`.

## Design

### Part 1: Well-Known Tagged Strings

Gram's `tagged_string` syntax is `tag`content`` — a symbol tag followed by a
backtick-quoted content string. The tag is any valid gram symbol; hyphens are
allowed (gram symbol regex: `[a-zA-Z_][0-9a-zA-Z_.\-@]*`), so JSON Schema
hyphenated format names like `date-time` are valid gram tags.

#### Temporal Tags

Tag names follow JSON Schema `format` vocabulary where an equivalent exists.
Neo4j-specific types without a JSON Schema counterpart use a hyphenated lowercase
form for consistency. All content strings are ISO 8601.

| Tag | Content format | Maps from | Notes |
|-----|---------------|-----------|-------|
| `date` | ISO 8601 date | JSON Schema / Neo4j `Date` | `date`2024-01-15`` |
| `time` | ISO 8601 time with UTC offset | JSON Schema / Neo4j `Time` | `time`10:30:00+01:00`` |
| `local-time` | ISO 8601 local time (no offset) | Neo4j `LocalTime` | `local-time`10:30:00`` — extends JSON Schema which always requires offset |
| `date-time` | ISO 8601 datetime with UTC offset | JSON Schema / Neo4j `DateTime` | `date-time`2024-01-15T10:30:00Z`` |
| `local-date-time` | ISO 8601 local datetime (no offset) | Neo4j `LocalDateTime` | `local-date-time`2024-01-15T10:30:00`` — extends JSON Schema |
| `duration` | ISO 8601 duration | JSON Schema / Neo4j `Duration` | `duration`P1Y2M3DT4H5M6S`` |

These are lossless round-trips for Neo4j: the ISO 8601 string is the canonical
serialization and the Cypher driver parses them back without coercion.

#### Spatial Tags

Spatial values use Extended WKT (EWKT) with explicit SRID — the OGC standard as
extended by PostGIS. Tag name: `point`.

| Neo4j CRS | SRID | Example |
|-----------|------|---------|
| WGS-84 2D | 4326 | `` point`SRID=4326;POINT(13.4 52.5)` `` |
| WGS-84 3D | 4979 | `` point`SRID=4979;POINT Z(13.4 52.5 34.0)` `` |
| Cartesian 2D | 7203 | `` point`SRID=7203;POINT(1.5 2.5)` `` |
| Cartesian 3D | 9157 | `` point`SRID=9157;POINT Z(1.5 2.5 3.5)` `` |

**Coordinate order**: WKT uses (longitude, latitude) for geographic CRS — the
reverse of Neo4j's `{latitude, longitude}` storage order. Implementations MUST
swap on export and re-swap on import.

#### Tier 1 — Required or broadly applicable

| Tag | Content format | Source |
|-----|---------------|--------|
| `uuid` | RFC 4122 UUID, lowercase hyphenated | JSON Schema `format` / RFC 4122 |
| `uri` | Absolute URI | JSON Schema `format` / RFC 3986 |
| `uri-reference` | URI or relative-reference | JSON Schema `format` / RFC 3986 |
| `iri` | Internationalized URI | JSON Schema `format` / RFC 3987 |
| `email` | Internet email address | JSON Schema `format` / RFC 5321 |
| `decimal` | Arbitrary-precision decimal (no exponent) | XML Schema `xs:decimal` |
| `markdown` | CommonMark string | CommonMark |

**Note on `decimal`**: Gram's native `Decimal` type is an `f64`, which is
sufficient for the vast majority of numeric values. The `decimal` tagged string
exists for portability with systems that use arbitrary-precision decimal (financial
data, certain scientific domains) — not because gram itself has a precision
limitation. It is defined here for completeness but is not required by `relate` v1.

**Note on `markdown` and `html`**: These are display-only tags with no content
validation requirement at this time. Both correspond to injectable tree-sitter
grammars (`tree-sitter-markdown`, `tree-sitter-html`) that could enable validation
later when there is demand. However, gram or Cypher code *inside* markdown
codefences IS in scope for `relate lint`: tree-sitter-markdown's codefence
injection mechanism makes `` ```gram `` and `` ```cypher `` fences parseable and
lintable within any markdown input — whether in a `.md` file or in a
`markdown`-tagged string property of a `.gram` file.

#### Tier 2 — Complete JSON Schema `format` coverage

Defined for namespace consistency; not required by `relate` v1.

| Tag | Content format | Source |
|-----|---------------|--------|
| `iri-reference` | IRI or relative-reference | JSON Schema `format` / RFC 3987 |
| `uri-template` | URI template | JSON Schema `format` / RFC 6570 |
| `idn-email` | Internationalized email | JSON Schema `format` / RFC 6531 |
| `hostname` | DNS hostname | JSON Schema `format` / RFC 1123 |
| `idn-hostname` | Internationalized hostname | JSON Schema `format` / RFC 5890 |
| `ipv4` | Dotted-quad IPv4 address | JSON Schema `format` |
| `ipv6` | IPv6 address | JSON Schema `format` / RFC 2373 |
| `json-pointer` | JSON Pointer | JSON Schema `format` / RFC 6901 |
| `relative-json-pointer` | Relative JSON Pointer | JSON Schema `format` |
| `regex` | ECMA-262 regular expression | JSON Schema `format` |
| `html` | HTML fragment | — |

#### Escalation Path: tree-sitter-gram and pattern-rs

**tree-sitter-gram** requires no grammar changes. The proposed escalation:

1. **Documentation**: A `WELL-KNOWN-TAGS.md` listing each tag, its content format,
   and its source type system.
2. **Syntax highlighting**: Entries in `queries/highlights.scm` giving well-known
   tags distinct highlighting from arbitrary user-defined tags.
3. **Validation queries** (future): Tree-sitter predicates flagging malformed content
   strings for a given tag.

**pattern-rs** (`gram_codec`) currently represents tagged strings as
`Value::TaggedString { tag: String, content: String }` — an open variant with no
type registry. The proposed escalation:

1. **Documentation**: A `WELL-KNOWN-TAGS.md` establishing the tag names as stable
   API surface.
2. **Decoding helpers**: A `decode_well_known()` method on `Value::TaggedString`
   returning a typed result for recognized tags, falling back gracefully for unknown
   tags. This preserves open-world semantics while giving callers a typed fast-path.
3. **Validation**: Content format validation in the decoding path with descriptive
   errors for malformed tagged values.

`relate` would be the first consumer, using the `pattern-rs` decoding path for
`.gram` import/export.

---

### Part 2: CSV ↔ gram

CSV has no type system — every value is a string. In the gram CSV workflow, type
meaning comes entirely from the user-supplied Cypher statements: a `CREATE (n:Person
{born: toInteger($born)})` statement is where `$born` gets coerced from string to
integer. The CSV layer imposes no type convention.

**CSV → gram (via Neo4j)**: The workflow is CSV → Neo4j (via Cypher) → gram. Once
values are in Neo4j with proper types, the Neo4j → gram export rules in Part 3 apply.

**gram → CSV**: Out of scope for v1. Gram types that have no CSV equivalent (range,
measurement, tagged strings) would require a lossy conversion strategy. Users who
need CSV output can query Neo4j directly.

**Conflict handling**: The user's Cypher statement determines merge vs. create
behavior (MERGE vs. CREATE). The tool does not impose a default strategy.

---

### Part 3: JSON ↔ gram

Gram's native type system covers most of JSON almost completely. The table below
shows the natural mappings and the gaps in both directions.

#### JSON → gram: Natural type mappings

| JSON type | Gram native equivalent | Notes |
|-----------|----------------------|-------|
| `boolean` | `boolean_literal` (`true`/`false`) | Full equivalence |
| `integer` | `integer` | Full equivalence; gram integers are signed arbitrary-size in notation, stored as i64 in `gram_codec` |
| `number` (float) | `decimal` | Gram `decimal` is f64; use `decimal` tagged string for arbitrary-precision values |
| `string` | `string_literal` | Full equivalence |
| `array` | `array` | Gram arrays may be heterogeneous |
| `object` | `record` or `map` | `record` for property values; `map` for standalone map values |
| `null` | *(none)* | Gram has no null literal; omit the property on export, treat missing property as null on import |

#### Gram → JSON: Types without JSON equivalents

Gram has several native types that JSON cannot represent:

| Gram type | Example | JSON treatment |
|-----------|---------|---------------|
| `range` | `5..10`, `5...`, `...10` | No JSON equivalent; serialize as tagged string `` range`5..10` `` or as a `{"min":5,"max":10}` object |
| `measurement` | `42km`, `3.5s` | No JSON equivalent; serialize as tagged string `` measurement`42km` `` or split into value + unit |
| `hexadecimal` | `0xFF` | Serialize as JSON integer (decimal) or as string `"0xFF"` |
| `octal` | `077` | Serialize as JSON integer (decimal) |
| `tagged_string` | `` date`2024-01-15` `` | Serialize using well-known tag conventions where defined; otherwise as plain string with loss of type information |
| `symbol` (as value) | `red` | Serialize as JSON string |

#### JSON Schema `format` → well-known tagged strings

JSON Schema `format` keywords annotate strings with semantic type information.
These map directly to the well-known tagged strings defined in Part 1. A JSON value
of `{"type": "string", "format": "date-time"}` containing `"2024-01-15T10:30:00Z"`
becomes the gram tagged string `` date-time`2024-01-15T10:30:00Z` ``.

---

### Part 4: Neo4j ↔ gram

#### Gram File Header Convention

A gram file optionally begins with a top-level record (the grammar's `root` field,
which must appear before any patterns if present). By convention this record
describes the file itself:

```gram
{ kind: social, graph: "customers" }
(alice:Person {name: "Alice", id: uuid`550e8400-e29b-41d4-a716-446655440000`})
(bob:Person {name: "Bob", id: uuid`...`})
(alice)-[:KNOWS]->(bob)
```

**Established `kind` values**:

| `kind` | Meaning |
|--------|---------|
| `social` | Graph data (nodes, relationships, properties) |
| `schema` | Graph schema descriptor (analogous to JSON Schema; work in progress) |

The `graph` property in the header record is the canonical **name** of the graph
the file represents. It is used by `put`/`get` to identify the named sub-graph in
the target database. It is optional — if absent, the filename stem is used as a
fallback.

**Header on export**: `relate get` always emits a header record containing at
minimum `{ kind: social, graph: "<name>" }`, making the exported file self-describing
and roundtrippable without relying on the filename.

#### Command Design

The previous proposal of `write --format gram` / `read --format gram` is superseded.
Two new commands, `put` and `get`, handle gram-as-graph-unit semantics:

- **`relate put <file.gram>`** — reads a `.gram` file, merges its patterns into the
  connected database as an upsert, and records membership of all imported nodes and
  relationships under the graph name resolved from the file header (or filename stem,
  or `--graph` flag).
- **`relate get [--graph <name>]`** — retrieves the named sub-graph from the
  connected database and serializes it as a `.gram` file with a complete header
  record. Writes to stdout or `--output`.

**Graph name resolution precedence** (highest to lowest):
1. `--graph <name>` flag (explicit override)
2. `graph` property in the file's top-level record
3. Filename stem (e.g., `customers.gram` → `customers`)

**MERGE semantics**: `put` always uses Cypher MERGE, producing upsert behavior —
existing nodes and relationships are updated with the properties from the gram file;
absent nodes and relationships are created. No `ON CREATE` or `ON MATCH` sub-clauses
are emitted by default.

The `--timestamps` flag enables conventional audit timestamp properties:

```
ON CREATE SET n.createdAt = datetime()
ON MATCH SET n.updatedAt = datetime()
```

Without `--timestamps`, the MERGE is a pure upsert with no side-effect properties.
Customizing the timestamp property names is out of scope for v1.

**Format generalization for `query`**: Rather than a dedicated `read` command,
`relate query` gains a `--format` flag for result serialization:

```
relate query "MATCH (n)-[r]->(m) RETURN n,r,m" --format gram
relate query "MATCH (n) RETURN n" --format json
relate query "MATCH (n) RETURN n.name, n.age" --format csv
```

When result columns are nodes or relationships the driver returns full graph objects
and gram/json serialization is natural. When columns are scalars the output is a
flat list of gram records or JSON objects respectively.

**GQL alignment path**: GQL (ISO/IEC 39075:2024) includes DDL for `CREATE GRAPH
TYPE` and `CREATE GRAPH`, and a `GRAPH <name>` clause for selecting among named
graph instances. However, no database has demonstrably implemented this in full.
Google Cloud Spanner has a `GRAPH FinGraph` clause but it references a **property
graph schema** (a named view/type defined with `CREATE PROPERTY GRAPH`) rather than
a runtime-created graph instance — closer to GQL's graph type than named graph
instances. Neo4j's GQL conformance docs list graph expressions (`CURRENT_GRAPH`,
`CURRENT_PROPERTY_GRAPH`) as unsupported; `CREATE GRAPH` is not tracked at all yet.

The `put`/`get` commands are designed so that if and when Neo4j does implement GQL
named graphs, the compatibility layer can be replaced: `put` would map to `CREATE
GRAPH <name>` and `get` to `GRAPH <name> MATCH (n)-[r]->(m) RETURN n,r,m`. The
gram file header `graph` property maps directly to the GQL graph name. But this is
a future alignment target, not an imminent migration path.

#### Sub-graph Membership Tracking (Pre-GQL Compatibility Layer)

Until Neo4j supports GQL named graphs natively, `put` must track which nodes and
relationships belong to a given named graph using available Neo4j mechanisms:

**Nodes**: Add the label `:_GramGraph_<name>` to every imported node. Neo4j indexes
labels efficiently, making membership queries fast. On `get`, the query is simply
`MATCH (n:_GramGraph_customers)`.

**Relationships**: Neo4j relationships cannot carry labels. Membership is tracked
via a manifest node: a single `(:_GramGraph {name: "<name>"})` node connected to
each member relationship's endpoints via `[:_GramMember]` edges that carry the
relationship's element ID. On `get`, the manifest node drives the retrieval query.

**Cleanup**: `relate put --replace <file.gram>` removes the existing membership
label/manifest before re-importing, providing a clean update path.

This layer is intentionally minimal — just enough to enable round-tripping. When
GQL named graphs arrive in Neo4j, the membership tracking can be migrated or
retired.

#### Identity Mapping

**Export (gram → Neo4j, via `put`)**: The user specifies `--id-property` naming the
node property to use as the gram identifier on both sides. If a node lacks that
property, the export fails with a clear error. There is no fallback to internal IDs.

When `--id-property` points to a UUID property the exported gram uses
`` uuid`550e8400-…` `` rather than a plain string, making the semantic role explicit.

**Import (Neo4j → gram, via `get`)**: The gram identifier is populated from
`--id-property`. If `--id-property` is omitted, the gram identifier falls back to
the default property name `_gramId`.

**Relationship identity**: Endpoints are matched by their `--id-property` value.
A missing endpoint fails the import with the identifier reported.

#### Neo4j Type Mapping

Neo4j types that have no gram native equivalent use well-known tagged strings from
Part 1:

| Neo4j type | Gram representation |
|------------|---------------------|
| `String` | `string_literal` |
| `Integer` | `integer` |
| `Float` | `decimal` |
| `Boolean` | `boolean_literal` |
| `List` | `array` |
| `Date` | `` date`…` `` |
| `Time` | `` time`…` `` |
| `LocalTime` | `` local-time`…` `` |
| `DateTime` | `` date-time`…` `` |
| `LocalDateTime` | `` local-date-time`…` `` |
| `Duration` | `` duration`…` `` |
| `Point` (WGS-84 2D) | `` point`SRID=4326;POINT(lon lat)` `` |
| `Point` (WGS-84 3D) | `` point`SRID=4979;POINT Z(lon lat h)` `` |
| `Point` (Cartesian 2D) | `` point`SRID=7203;POINT(x y)` `` |
| `Point` (Cartesian 3D) | `` point`SRID=9157;POINT Z(x y z)` `` |

**Out of scope for v1**: Neo4j map properties (stored as JSON), byte arrays, and
node/relationship types as first-class values. Annotations and generic patterns
(gram-only features) are not produced on export and cause a descriptive error on
import.

## Unresolved Questions

- **`put --replace` semantics**: Deferred from v1. `put` is MERGE/upsert only for
  now. Replace semantics (remove prior membership, re-import clean) can be added
  once the basic round-trip is validated in practice.

- **`_gramId` as default id-property**: Should the default `--id-property` fallback
  (`_gramId`) be configurable? Out of scope for v1; revisit if users request it.

- **Annotations → Neo4j properties**: Can gram annotations be flattened into Neo4j
  node properties on import rather than rejected? Potential v2 extension; requires
  a flattening convention.

- **Nodes missing `--id-property`**: Fail the entire export, skip with a warning,
  or auto-generate identifiers? Current proposal: fail fast. Validate against
  real-world usage.

- **Escalation sequencing for well-known tags**: Should `relate` ship with its own
  internal tag decoding first, then propose upstream once validated? Or propose
  upstream first? Lower risk to ship internally first; proposing upstream first
  avoids convention fragmentation.

- **`pattern-rs` decode API shape**: Should `decode_well_known()` return a
  `WellKnownValue` enum, or per-tag accessor methods (`as_uuid()`, `as_date()`, etc.)?
  Enum is more ergonomic for exhaustive matching; named methods are easier to extend.

- **`decimal` canonical form**: Should the spec mandate a canonical form (e.g.,
  always include a decimal point: `1.0` not `1`) for round-trip stability?

- **`range` serialization for JSON**: Tagged string (`` range`5..10` ``) preserves
  gram semantics; `{"min":5,"max":10}` object is more natural for JSON consumers.
  Should a canonical JSON encoding for gram ranges be defined here?

- **`kind: schema` gram files**: The emerging `{ kind: schema }` convention for
  graph schema descriptors is related to this RFC's type mapping work but out of
  scope. It should be tracked as a separate RFC when the schema format stabilizes.
