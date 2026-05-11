# relate

CLI for working with `.cypher` and `.gram` files and Neo4j.

## Status

Early development. See [`proposals/`](proposals/) for the RFC roadmap.

## Installation

**macOS / Linux — shell script (recommended):**
```bash
curl -sSfL https://cli.relateby.dev/install.sh | bash
```

**Homebrew (macOS):**
```bash
brew install relateby/tap/relate
```

**npm (all platforms):**
```bash
npm i -g @relateby/cli
```

**cargo (from source):**
```bash
cargo install relate
```

> **macOS Gatekeeper note:** If you manually download a binary from the GitHub Releases
> page and macOS blocks it, run:
> ```bash
> xattr -d com.apple.quarantine /path/to/relate
> ```
> This is not needed for `curl | bash`, Homebrew, or npm installs.

## Usage

```
relate --help

relate lint query.cypher
relate parse schema.gram
relate write --password $NEO4J_PASSWORD nodes.gram
relate read --password $NEO4J_PASSWORD "MATCH (n) RETURN n LIMIT 10"
relate mcp ./queries/
```

## Development

```bash
cargo build
cargo test
cargo clippy --all-targets
cargo fmt
```

## RFC Process

New features are proposed in `proposals/RFC-NNN.md`. See [RFC-001](proposals/RFC-001.md) for
the RFC format specification and project goals. Each RFC maps to a speckit feature branch
named `NNN-short-name`.
