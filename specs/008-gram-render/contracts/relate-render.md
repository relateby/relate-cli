# CLI Contract: `relate render`

**Branch**: `008-gram-render` | **Date**: 2026-05-14

## Synopsis

```
relate render [OPTIONS] <FILE>
```

## Arguments

| Argument | Type | Required | Description |
|----------|------|----------|-------------|
| `<FILE>` | path | yes | Path to a `.gram` file to render |

## Options

| Flag | Short | Type | Default | Description |
|------|-------|------|---------|-------------|
| `--format <FORMAT>` | | `html\|svg` | `html` | Output format |
| `--output <FILE>` | `-o` | path | `<input-stem>.<format>` | Output file path |
| `--open` | | bool | false | Open output in system default viewer after writing |
| `--json` | | bool | false | Print result as JSON to stdout (see below) |
| `--help` | `-h` | | | Print help |

## Exit Codes

| Code | Meaning |
|------|---------|
| 0 | Success — output file written |
| 1 | Parse error — input is not valid gram |
| 2 | I/O error — cannot read input or write output |

## Standard Streams

- **stdin**: Not read. `relate render` requires a file path argument.
- **stdout**: Silent on success unless `--json` is passed. With `--json`, prints a
  single JSON object (see below).
- **stderr**: Human-readable error message on failure. Nothing on success.

## JSON Output (`--json`)

On **success**:
```json
{"output": "path/to/output.html", "format": "html"}
```

On **failure**:
```json
{"error": "parse error at line 3: unexpected token '}'"}
```

The JSON object is always a single line followed by a newline. No other output is
written to stdout when `--json` is active.

## Examples

```bash
# Interactive HTML visualization (default)
relate render graph.gram

# Static SVG for documentation
relate render graph.gram --format svg

# Explicit output path
relate render graph.gram --output docs/diagram.svg --format svg

# Open immediately after rendering
relate render graph.gram --open

# Machine-readable output path (for scripting)
relate render graph.gram --json
# stdout: {"output":"graph.html","format":"html"}

# Use in a pipeline
OUTPUT=$(relate render graph.gram --format svg --json | jq -r .output)
cp "$OUTPUT" docs/
```

## Behaviour Details

- If `--output` is omitted, the output file is written to the same directory as
  the input file, with the input's stem and the format extension.
  Example: `relate render foo/bar.gram --format svg` → `foo/bar.svg`.
- If the output file already exists, it is overwritten without warning.
- `--open` uses the system default file association (`xdg-open` / `open` /
  `start`). It is a best-effort flag; failure to open does not change the exit
  code.
- `--json` is compatible with all other flags. When combined with `--open`,
  the file is opened and the JSON is also printed.
