# Feature Specification: Packaging and Distribution

**Feature Branch**: `005-packaging-distribution`
**Created**: 2026-05-11
**Status**: Draft
**Input**: User description: "packaging and distribution of the relate-cli as described in proposals/RFC-005-distribution.md"

## User Scenarios & Testing *(mandatory)*

### User Story 1 — One-liner install on macOS or Linux (Priority: P1)

A developer hears about `relate` from a colleague or the website and wants to try it immediately. They run a single `curl | bash` command and have a working binary in their PATH within seconds.

**Why this priority**: The `curl | bash` path is the fastest route from "heard about it" to "running it." If this doesn't work reliably, no other distribution channel matters for first-time users.

**Independent Test**: Run `curl -sSfL https://cli.relateby.dev/install.sh | bash` on a fresh macOS and a fresh Ubuntu environment; confirm `relate --version` works immediately after.

**Acceptance Scenarios**:

1. **Given** a macOS (Intel or Apple Silicon) machine with `curl` installed, **When** the user runs `curl -sSfL https://cli.relateby.dev/install.sh | bash`, **Then** the `relate` binary is installed to `~/.local/bin` and `relate --version` prints the current version.
2. **Given** an Ubuntu or Debian Linux machine, **When** the user runs the same command, **Then** the correct Linux binary (x86_64 or aarch64) is installed and executable.
3. **Given** the install directory is not in `$PATH`, **When** the script completes, **Then** it prints shell-specific setup instructions for adding the directory to `$PATH`.
4. **Given** a sha256 mismatch on the downloaded archive, **When** the script verifies the checksum, **Then** installation is aborted with a clear error message.

---

### User Story 2 — Install via Homebrew (Priority: P2)

A macOS developer prefers to manage all their tools with Homebrew and wants `relate` to behave like any other `brew install` package — upgradeable with `brew upgrade`, removable with `brew uninstall`.

**Why this priority**: Homebrew is the dominant CLI package manager on macOS. Being in a tap gives `relate` discoverability and a familiar upgrade path.

**Independent Test**: Run `brew install relateby/tap/relate` on a macOS machine with no prior `relate` installation; confirm the binary works and `brew upgrade relate` succeeds after a new release.

**Acceptance Scenarios**:

1. **Given** a macOS machine with Homebrew installed, **When** the user runs `brew install relateby/tap/relate`, **Then** the `relate` binary is installed and `relate --version` prints the correct version.
2. **Given** a new `relate` stable release is published, **When** the user runs `brew upgrade relate`, **Then** the updated binary is installed.
3. **Given** a prerelease tag is pushed, **When** the release pipeline runs, **Then** the Homebrew formula is NOT updated (stable-only formula).

---

### User Story 3 — Install via npm (Priority: P3)

A JavaScript or TypeScript developer already uses npm globally for CLI tools and wants to install `relate` without needing a separate package manager.

**Why this priority**: npm reaches a large audience of frontend developers and AI tooling authors who may not use Homebrew or compile Rust. The `optionalDependencies` pattern avoids postinstall script failures in restricted CI environments.

**Independent Test**: Run `npm i -g @relateby/cli` on macOS, Linux, and Windows; confirm `relate --version` works on each platform without any postinstall scripts executing.

**Acceptance Scenarios**:

1. **Given** a machine with Node.js installed, **When** the user runs `npm i -g @relateby/cli`, **Then** only the platform-appropriate binary package is downloaded and `relate` is available in PATH.
2. **Given** a CI environment running with `--ignore-scripts`, **When** the user installs `@relateby/cli`, **Then** installation succeeds without any postinstall scripts.
3. **Given** a prerelease is available, **When** the user runs `npm i -g @relateby/cli@next`, **Then** the prerelease binary is installed.

---

### User Story 4 — Project website as the install hub (Priority: P2)

A user lands on `cli.relateby.dev` from a README link or web search. The page shows the install command for their OS automatically, lets them copy it, and explains what `relate` does with live command examples.

**Why this priority**: The website is the top-of-funnel for all install paths. A clear, fast, correct landing page directly drives adoption.

**Independent Test**: Open `cli.relateby.dev` on macOS (Safari), Linux (Chrome), and Windows (Chrome); verify the OS-specific install tab is pre-selected and the copy button works.

**Acceptance Scenarios**:

1. **Given** a user opens the website on macOS, **When** the page loads, **Then** the macOS install tab is pre-selected and shows the `curl | bash` command.
2. **Given** a new stable release is published, **When** the `update-website.yml` workflow runs, **Then** the version shown on the website and the `install.sh` served at `cli.relateby.dev/install.sh` both reflect the new version within 5 minutes.
3. **Given** a user clicks the copy button on an install command, **When** they paste it into a terminal, **Then** the exact correct command is pasted with no extra whitespace or characters.
4. **Given** a user on a mobile device visits the website, **When** the page renders, **Then** all content is readable and functional at 375px viewport width.

---

### User Story 5 — Automated release pipeline (Priority: P1)

A maintainer pushes a version tag `v0.2.0` and all distribution artifacts — GitHub Release archives, Homebrew formula, npm packages, crates.io crate — are published automatically without any manual steps.

**Why this priority**: Manual release steps introduce errors and slow the release cadence. The entire pipeline must be trigger-once and self-healing.

**Independent Test**: Push a `v0.x.y` tag on a branch; observe all four channels (GitHub Releases, Homebrew tap, npm, crates.io) update correctly without any manual intervention.

**Acceptance Scenarios**:

1. **Given** a `vX.Y.Z` tag is pushed to `main`, **When** the release workflow runs, **Then** GitHub Release archives for all 5 platforms are published with sha256 checksums.
2. **Given** the same stable tag, **When** the workflow completes, **Then** `brew install relateby/tap/relate` installs the new version.
3. **Given** the same stable tag, **When** `npm i -g @relateby/cli` runs, **Then** the new version is installed.
4. **Given** the same stable tag, **When** the workflow completes, **Then** `cargo install relate` installs the new version from crates.io.
5. **Given** a prerelease tag `vX.Y.Z-beta.N`, **When** the workflow runs, **Then** GitHub Release and npm `@next` are updated but the Homebrew formula and crates.io are NOT.

---

### Edge Cases

- What happens when the release tag version does not match `version` in `Cargo.toml`? → The workflow should fail fast with a clear error before any artifacts are published.
- What happens when the `HOMEBREW_TAP_TOKEN` secret is missing or expired? → The release workflow should fail the Homebrew step but still publish GitHub Release and npm artifacts; the failure must be clearly reported.
- What happens when a user's `~/.local/bin` does not exist? → The install script creates it before placing the binary.
- What happens when a user runs the install script twice? → The existing binary is replaced with the new version.
- What happens when a user manually downloads a `.tar.gz` from the GitHub Releases page on macOS? → Gatekeeper quarantines it; the website and README document the `xattr -d com.apple.quarantine` workaround.

## Requirements *(mandatory)*

### Functional Requirements

- **FR-001**: The project MUST publish prebuilt binaries for all 5 targets (Linux x86_64, Linux ARM64, macOS x86_64, macOS ARM64, Windows x86_64) on every release.
- **FR-002**: Every release MUST include a sha256 checksum file and the install script MUST verify it before installing.
- **FR-003**: The install script MUST be served at `https://cli.relateby.dev/install.sh` and MUST always reflect the latest stable release.
- **FR-004**: The Homebrew tap formula MUST be updated automatically on every stable release and MUST NOT be updated on prerelease tags.
- **FR-005**: npm packages MUST use `optionalDependencies` for platform selection and MUST NOT execute any postinstall scripts.
- **FR-006**: The release pipeline MUST be triggered solely by pushing a version tag (`vX.Y.Z` or `vX.Y.Z-<prerelease>`) to `main`; no manual workflow dispatch is required for normal releases.
- **FR-007**: The website at `cli.relateby.dev` MUST auto-detect the visitor's OS and pre-select the appropriate install tab.
- **FR-008**: The website MUST be updated automatically within 5 minutes of a new stable release being published.
- **FR-009**: The website MUST serve `install.sh` and `install.ps1` from the same domain (`cli.relateby.dev`) rather than redirecting to GitHub.
- **FR-010**: `cargo publish` MUST run as part of the stable release pipeline, making the crate available via `cargo install relate`.
- **FR-011**: Prerelease builds MUST be available via `npm i -g @relateby/cli@next` and MUST be marked as prerelease on GitHub Releases.
- **FR-012**: The release workflow MUST fail fast if the `Cargo.toml` version does not match the pushed tag.

### Key Entities

- **GitHub Release**: A tagged release containing per-platform archives (`.tar.gz` / `.zip`), a `dist-manifest.json`, and a `checksums.txt` file.
- **Homebrew Tap** (`relateby/homebrew-tap`): A public repository hosting `Formula/relate.rb`, maintained automatically by the release pipeline.
- **npm wrapper package** (`@relateby/cli`): A thin Node.js shim package that depends on platform packages via `optionalDependencies`.
- **npm platform package** (`@relateby/cli-<platform>`): A package containing a prebuilt binary for a specific OS/arch combination, with `os`/`cpu` constraints in `package.json`.
- **Website** (`cli.relateby.dev`): A static site served from the `gh-pages` branch with OS-aware install instructions and command examples.
- **Install script** (`install.sh` / `install.ps1`): Shell scripts generated by cargo-dist that download and verify the correct binary for the current platform.

## Success Criteria *(mandatory)*

### Measurable Outcomes

- **SC-001**: A user on macOS, Linux (x86_64), or Linux (ARM64) can go from zero to a working `relate` binary using `curl | bash` in under 60 seconds on a standard broadband connection.
- **SC-002**: A maintainer can publish a complete release to all four distribution channels (GitHub Releases, Homebrew, npm, crates.io) by pushing a single version tag — no manual steps required.
- **SC-003**: The website version badge and served `install.sh` are updated to reflect a new release within 5 minutes of the GitHub Release being published.
- **SC-004**: `npm i -g @relateby/cli` succeeds in a `--ignore-scripts` environment on all supported platforms.
- **SC-005**: All five platform archives include a verified sha256 checksum and the install script refuses to proceed if the checksum fails.
- **SC-006**: The website renders correctly on desktop (1280px+) and mobile (375px) viewports in both light and dark modes.

## Assumptions

- The `relateby` GitHub organization exists and the maintainer has admin access to create repositories and manage secrets.
- The `@relateby` npm scope has been claimed (or will be claimed as a prerequisite step).
- The `cli.relateby.dev` subdomain is already registered (GoDaddy) and the CNAME record will be pointed to `relateby.github.io`.
- Windows ARM64 is out of scope for v1 (only x86_64 Windows is targeted).
- macOS code signing and notarization are out of scope; the `xattr` workaround is acceptable for users who manually download from GitHub Releases.
- Windows Authenticode signing is out of scope for v1.
- `cargo-dist` is used as the primary release automation tool; it generates both the cross-compiled archives and the install scripts.
- A fine-grained GitHub PAT (not a GitHub App) is sufficient for pushing the Homebrew tap formula, per cargo-dist's documented approach.
- The website uses no JavaScript framework — vanilla HTML/CSS/JS only — to minimize maintenance burden and maximize page speed.
- `CHANGELOG.md` is maintained by hand for v1; structured changelog tooling (changie) is deferred.
