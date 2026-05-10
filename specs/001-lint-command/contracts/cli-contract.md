# CLI Contract: relate lint

## Command Signature

```
relate lint [OPTIONS] [FILES]...
```

## Arguments

| Argument | Type | Description |
|----------|------|-------------|
| `FILES`  | `Vec<PathBuf>` (0+) | Files or directories to lint. Reads from stdin when omitted. |

## Options

| Flag | Short | Type | Default | Description |
|------|-------|------|---------|-------------|
| `--expr` | `-e` | `String` | — | Lint an inline expression instead of a file |
| `--lang` | — | `cypher\|gram` | `cypher` | Engine for `--expr` or stdin |
| `--json` | — | bool | false | Output diagnostics as JSON |
| `--strict` | — | bool | false | Treat warnings as errors |

## Exit Codes

| Code | Condition |
|------|-----------|
| `0` | No errors found (no warnings in `--strict` mode) |
| `1` | One or more lint errors (or warnings under `--strict`) found |
| `2` | relate itself failed: I/O error, unsupported file type given explicitly, or other tool error |

## Supported File Types

| Extension | Engine |
|-----------|--------|
| `.cypher` | cypher-data |
| `.gram` | gram-data |
| `.md` | cypher-data (cypher/openCypher fences), gram-data (gram fences) |
| `.adoc` | cypher-data (source,cypher blocks), gram-data (source,gram blocks) |
| other (explicit) | exit code 2 |
| other (directory walk) | silently skipped |

## Human-Readable Output Format

Diagnostics grouped by source file, rendered via ariadne:

```
queries/find-path.cypher:3:12: warning [structural/unbounded_relationship]
  Relationship has no upper bound on path length
```

No output when there are no diagnostics.

## JSON Output Schema (`--json`)

Array of objects. Empty array `[]` when no diagnostics. Always valid JSON even on clean input.

```json
[
  {
    "severity": "warning",
    "rule": "structural/unbounded_relationship",
    "message": "Relationship has no upper bound on path length",
    "code": "W001",
    "file": "queries/find-path.cypher",
    "range": {
      "start": { "line": 3, "column": 12 },
      "end":   { "line": 3, "column": 20 }
    }
  }
]
```

Fields:
- `severity`: one of `"error"`, `"warning"`, `"information"`, `"hint"`
- `rule`: rule identifier string
- `message`: human-readable description
- `code`: optional rule code; omitted if absent
- `file`: source file path, or `null` for `--expr`/stdin
- `range.start.line`, `range.start.column`: 0-based
- `range.end.line`, `range.end.column`: 0-based

## Fence Language Tags (Markdown / AsciiDoc)

| Tag | Engine |
|-----|--------|
| `cypher`, `openCypher` (case-insensitive) | cypher-data |
| `gram` | gram-data |
| anything else | silently skipped |

Diagnostics from fenced snippets report line numbers relative to the enclosing document.
