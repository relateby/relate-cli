# Feature Specification: relate lint Command

**Feature Branch**: `001-lint-command`  
**Created**: 2026-05-10  
**Status**: Draft  
**RFC**: proposals/RFC-002.md

## User Scenarios & Testing *(mandatory)*

### User Story 1 - Lint a Cypher File (Priority: P1)

A developer has written Cypher queries in `.cypher` files and wants to catch structural
or stylistic issues before committing or running them against a database. They run
`relate lint` on one or more files and see actionable diagnostics with file, line,
and column references.

**Why this priority**: The primary use case — validating Cypher is the core job of the tool.

**Independent Test**: Can be fully tested by running `relate lint <file>.cypher` against a file
containing a known lint violation and verifying the diagnostic is reported with the correct
location and exit code 1.

**Acceptance Scenarios**:

1. **Given** a `.cypher` file with no violations, **When** the user runs `relate lint file.cypher`, **Then** the command exits with code 0 and produces no output.
2. **Given** a `.cypher` file with a lint violation, **When** the user runs `relate lint file.cypher`, **Then** the command prints the diagnostic (file, line, column, severity, message) and exits with code 1.
3. **Given** a non-existent file, **When** the user runs `relate lint missing.cypher`, **Then** the command prints an error and exits with code 2.

---

### User Story 2 - Lint a Gram File (Priority: P1)

A developer has written Gram graph notation in `.gram` files and wants to validate them
for correctness before use.

**Why this priority**: Equally primary to Cypher linting — both are first-class file types.

**Independent Test**: Can be fully tested by running `relate lint <file>.gram` against a file
with a known violation and verifying the diagnostic.

**Acceptance Scenarios**:

1. **Given** a `.gram` file with no violations, **When** the user runs `relate lint file.gram`, **Then** the command exits with code 0 and produces no output.
2. **Given** a `.gram` file with a violation, **When** the user runs `relate lint file.gram`, **Then** the diagnostic is printed with correct location and exit code 1.

---

### User Story 3 - Lint a Mixed Project Directory (Priority: P2)

A developer has a project directory containing a mix of `.cypher`, `.gram`, `.md`, and
other files. They want to lint the entire project in one command, seeing all issues
across all supported file types grouped by file.

**Why this priority**: The main productivity multiplier — eliminates running separate commands per file type.

**Independent Test**: Can be tested by running `relate lint <dir>` against a directory with
at least one `.cypher` and one `.gram` file, both with violations, and verifying both are reported.

**Acceptance Scenarios**:

1. **Given** a directory with `.cypher` and `.gram` files, **When** the user runs `relate lint <dir>`, **Then** diagnostics for all files are printed, grouped by file, and the exit code is 1 if any errors exist.
2. **Given** a directory containing unsupported file types (e.g., `.json`, `.rs`), **When** the user runs `relate lint <dir>`, **Then** unsupported files are silently skipped and only `.cypher`, `.gram`, `.md`, and `.adoc` files are processed.
3. **Given** a directory with no violations, **When** the user runs `relate lint <dir>`, **Then** the command exits with code 0 and produces no output.

---

### User Story 4 - Lint an Inline Expression (Priority: P2)

A developer wants to quickly check a Cypher or Gram snippet without writing it to a file,
for example while experimenting in a shell session.

**Why this priority**: Useful for quick checks; a common shell workflow pattern.

**Independent Test**: Can be tested by running `relate lint --expr "MATCH (n) RETURN n"` and
verifying the correct engine is selected and the result is reported.

**Acceptance Scenarios**:

1. **Given** a valid Cypher expression, **When** the user runs `relate lint --expr "MATCH (n) RETURN n"`, **Then** the command exits with code 0.
2. **Given** a Gram expression with `--lang gram`, **When** the user runs `relate lint --lang gram --expr "(a)-[r]->(b)"`, **Then** the Gram engine is used and the result is reported.
3. **Given** an invalid expression, **When** the user runs `relate lint --expr "<invalid>"`, **Then** a diagnostic is printed and the exit code is 1.

---

### User Story 5 - Lint Embedded Code Snippets in Markdown Docs (Priority: P2)

A developer maintains documentation (Markdown or AsciiDoc) that contains fenced Cypher
and Gram code blocks. They want to verify those snippets are valid without extracting
them manually.

**Why this priority**: Docs with broken examples are a common source of confusion; automating this saves significant manual effort.

**Independent Test**: Can be tested by running `relate lint doc.md` against a Markdown file
containing a fenced Cypher block with a violation and verifying the diagnostic references
the correct line in the `.md` file (not line 1 of the snippet).

**Acceptance Scenarios**:

1. **Given** a Markdown file with a fenced `cypher` block containing a violation, **When** the user runs `relate lint doc.md`, **Then** the diagnostic references the line number within the Markdown file, not the snippet.
2. **Given** a Markdown file with fenced blocks of unsupported languages, **When** the user runs `relate lint doc.md`, **Then** those fences are silently skipped.
3. **Given** an AsciiDoc file with `[source,cypher]` blocks, **When** the user runs `relate lint doc.adoc`, **Then** the embedded snippets are linted with correct line references.

---

### User Story 6 - CI Integration with JSON Output (Priority: P2)

A CI pipeline wants to consume lint results programmatically — for example, to annotate
pull requests or produce a structured report.

**Why this priority**: Structured output is required for automated tooling integration.

**Independent Test**: Can be tested by running `relate lint --json <file>` and verifying
the output is valid JSON matching the specified schema.

**Acceptance Scenarios**:

1. **Given** a file with violations, **When** the user runs `relate lint --json file.cypher`, **Then** the output is a JSON array of diagnostic objects with `severity`, `rule`, `message`, `file`, and `range` fields.
2. **Given** a file with no violations, **When** the user runs `relate lint --json file.cypher`, **Then** the output is an empty JSON array `[]` and exit code is 0.

---

### User Story 7 - Strict Mode for CI Gatekeeping (Priority: P3)

A team wants their CI pipeline to fail on any warnings, not just errors, to enforce
higher code quality standards.

**Why this priority**: Policy enforcement; useful but optional — teams can adopt it independently.

**Independent Test**: Can be tested by running `relate lint --strict` against a file with
only warnings and verifying exit code 1.

**Acceptance Scenarios**:

1. **Given** a file with only warnings (no errors), **When** the user runs `relate lint --strict file.cypher`, **Then** the command exits with code 1.
2. **Given** the same file without `--strict`, **When** the user runs `relate lint file.cypher`, **Then** the command exits with code 0.

---

### Edge Cases

- What happens when stdin is piped and no `--lang` is specified? Default to `cypher`.
- What happens when a Markdown file contains only fenced blocks of unsupported languages? Exit code 0, no output.
- What happens when a directory contains no supported files? Exit code 0, no output.
- What happens when a file has mixed severities and `--strict` is set? Exit code 1; all diagnostics (errors and warnings) are reported.
- What happens with deeply nested directory trees? Walk recursively with no depth limit.

## Requirements *(mandatory)*

### Functional Requirements

- **FR-001**: The command MUST accept one or more files or directories as positional arguments, and MUST read from stdin when no arguments are provided.
- **FR-002**: The command MUST dispatch `.cypher` files to the Cypher lint engine.
- **FR-003**: The command MUST dispatch `.gram` files to the Gram lint engine.
- **FR-004**: The command MUST walk directories recursively, dispatching each `.cypher`, `.gram`, `.md`, and `.adoc` file to the appropriate engine; all other file types MUST be silently skipped.
- **FR-005**: The command MUST extract code-fenced snippets from `.md` and `.adoc` files using the language tags `cypher`, `openCypher` (case-insensitive), and `gram`, then lint each snippet with the appropriate engine.
- **FR-006**: Diagnostics from fenced snippets MUST report line numbers relative to the enclosing document, not relative to the snippet.
- **FR-007**: The `--expr` flag MUST accept an inline expression and lint it using the engine selected by `--lang` (default: `cypher`).
- **FR-008**: The `--json` flag MUST produce a JSON array of diagnostic objects; human-readable output MUST be the default.
- **FR-009**: The `--strict` flag MUST cause the command to exit with code 1 when any warnings are present, even if no errors are found.
- **FR-010**: Exit codes MUST be: 0 (no errors/warnings under strict), 1 (lint findings), 2 (tool failure such as I/O error or unsupported file type passed explicitly).
- **FR-011**: Human-readable output MUST include file path, line number, column number, severity, rule identifier, and message for each diagnostic.
- **FR-012**: Human-readable output MUST be grouped by source file.

### Key Entities

- **Diagnostic**: A single lint finding — severity, rule name, human-readable message, optional rule code, and a range (start/end line and column) within a source file.
- **Snippet**: A code-fenced block extracted from a documentation file — source text plus the 0-based line offset of its opening fence within the parent document.

## Success Criteria *(mandatory)*

### Measurable Outcomes

- **SC-001** *(post-v1)*: Running `relate lint` on a file with a known violation produces output within 2 seconds on a modern laptop for files up to 10,000 lines. Validated by benchmark, not automated test.
- **SC-002** *(post-v1)*: Running `relate lint` on a directory with 500 files (`.cypher`, `.gram`, `.md` mixed) completes within 30 seconds. Validated by benchmark, not automated test.
- **SC-003**: The `--json` output is valid JSON parseable by standard tooling with no schema deviations.
- **SC-004**: Diagnostic line numbers for snippets embedded in Markdown are accurate to the actual line in the document in 100% of cases.
- **SC-005**: Exit codes are correct in 100% of tested scenarios (clean file → 0, violation → 1, I/O error → 2).

## Assumptions

- Target users are developers familiar with command-line tools and the Cypher/Gram languages.
- File encoding is UTF-8; other encodings are out of scope for v1.
- The Cypher and Gram lint engines handle their own parse errors as diagnostics; `relate lint` does not need to implement language parsing.
- AsciiDoc fence detection covers `[source,cypher]` / `[source,gram]` followed by `----` delimited blocks; other AsciiDoc block styles are out of scope for v1.
- Parallel/concurrent file processing is out of scope for v1.
- Custom lint rule directories are out of scope for v1; only the rules embedded in the upstream engines are used.
- `serde` / JSON serialization of the shared `gram-diagnostics` type is available or can be derived.
