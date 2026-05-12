<!--
SYNC IMPACT REPORT
==================
Version change: (none — initial ratification) → 1.0.0
Principles added:
  - I. CLI-Friendly
  - II. Human-Readable
  - III. Agent-Friendly
  - IV. Self-Contained Help
Sections added:
  - Core Principles (4 principles)
  - Development Standards
  - Governance
Templates reviewed:
  - .specify/templates/plan-template.md — Constitution Check section uses "[Gates determined based
    on constitution file]"; gates now derivable from the 4 principles below. ✅ no update needed
  - .specify/templates/spec-template.md — no constitution references. ✅ no update needed
  - .specify/templates/tasks-template.md — no constitution references. ✅ no update needed
Follow-up TODOs: none
-->

# relate Constitution

## Core Principles

### I. CLI-Friendly

Every command MUST behave as a well-mannered Unix citizen:

- Reads from stdin when no file arguments are given; writes results to stdout; writes errors to stderr.
- Returns meaningful exit codes: 0 for success, non-zero for distinct failure modes (see each command's contract).
- Accepts multiple files or directories; processes them without side effects on other files.
- Produces no interactive prompts during normal operation; all configuration comes from flags or environment variables.

Rationale: `relate` is intended to compose with other tools (editors, CI pipelines, shell scripts). Pipeable I/O and predictable exit codes are the price of entry.

### II. Human-Readable

Default output MUST be readable by a person at a glance:

- Diagnostic and result output uses structured, indented, or annotated formatting (ariadne for lint; s-expressions or similar for parse output).
- Errors explain what went wrong and, where possible, how to fix it.
- Color and emphasis are used where the terminal supports them; output degrades gracefully when piped or redirected.

Rationale: Developers spend more time reading output than writing commands. Cryptic output wastes time.

### III. Agent-Friendly

Every command MUST be usable by automated agents and LLM-based tools:

- Structured JSON output is available via a `--json` flag; the schema is stable and documented in the command's CLI contract.
- Commands relevant to graph/Cypher work MUST be exposed as MCP tools (via `relate mcp`).
- New commands MUST ship with `skills/relate-<command>/SKILL.md` before the implementation is considered complete. This file is the agent-discovery surface for the command and MUST include: YAML frontmatter (`name`, `description`, `triggers`), a numbered workflow with checkpoints, exit criteria, and an anti-rationalization table. The `skills/relate/SKILL.md` routing table and available-skills list MUST also be updated.

Rationale: `relate` is used in both human and automated workflows. A command that can only be used interactively is half-finished.

### IV. Self-Contained Help

Every command MUST be fully self-documenting:

- `--help` for every subcommand includes: purpose, all flags with defaults, at least one usage example, and supported file types or input formats.
- No external documentation should be required to use a command for the first time.
- Help text is kept in sync with actual behavior; drift is a bug.

Rationale: Developers discover `relate` commands through `--help`, not docs sites. Self-contained help is the primary UX surface.

## Development Standards

- **RFC before feature**: Any new command or significant behavior change MUST be preceded by an RFC in `proposals/`. Numbering is sequential (RFC-001, RFC-002, …).
- **Sync commands**: Lint and parse commands are synchronous (`fn run`, no `async`). Write, read, and mcp commands are async (via tokio). Do not introduce async into sync commands without justification.
- **No hardcoded credentials**: Neo4j passwords and other secrets MUST come from flags (`--password`) or environment variables (`NEO4J_PASSWORD`). Never commit credentials.
- **Single binary, no daemon**: `relate` ships as a single self-contained binary. No background processes, no local config files created by default, no network calls except those explicitly requested by the user.

## Governance

This constitution supersedes informal conventions. Amendments require:
1. A proposal describing the change and its motivation (RFC or PR description).
2. A version bump following semantic versioning: MAJOR for removals or redefinitions, MINOR for new principles or sections, PATCH for clarifications.
3. Update of this file and any templates that reference the changed principles.

All implementation plans MUST include a Constitution Check section verifying the feature does not violate the four core principles. Violations must be explicitly justified in a Complexity Tracking table.

**Version**: 1.0.0 | **Ratified**: 2026-05-10 | **Last Amended**: 2026-05-10
