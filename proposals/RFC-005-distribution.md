---
number: "005"
title: "Packaging, Distribution, and Project Website"
status: "Draft"
date: "2026-05-11"
authors:
  - "Andreas Kollegger <andreas.kollegger@neo4j.com>"
---

# RFC-005: Packaging, Distribution, and Project Website

## Summary

This RFC defines the packaging, release, distribution, and website strategy for `relate`. It establishes how users install the tool across platforms, how releases are automated, and how a project website hosted on GitHub Pages stays in sync with each release.

The approach is modeled closely on [neo4j-cli](https://github.com/neo4j-labs/neo4j-cli) ([neo4j.sh](https://neo4j.sh)), adapted for a single-crate Rust binary using [cargo-dist](https://github.com/axodotdev/cargo-dist) instead of GoReleaser.

---

## Motivation

`relate` is currently buildable from source only. To reach Neo4j developers, data engineers, and AI-agent authors who will not compile from source, we need:

1. A one-liner install command (`curl | bash`) backed by a memorable domain.
2. Package manager entries so experienced developers find `relate` where they already look (Homebrew, npm, `cargo install`).
3. An automated release pipeline so a version tag is all it takes to ship to every channel.
4. A project website that documents installation and shows the tool in action.

The neo4j-cli project solved the identical problem for a Go binary and has reached broad adoption. We reuse their proven distribution patterns and adapt the Rust-specific tooling.

---

## Decisions

| Question | Decision |
|----------|----------|
| Domain | `cli.relateby.dev` (registered on GoDaddy) |
| npm scope | `@relateby` |
| Homebrew credentials | Fine-grained PAT stored as `HOMEBREW_TAP_TOKEN` secret (see §2) |
| macOS signing | Not required — all primary install paths bypass Gatekeeper (see §Appendix A) |
| Windows signing | Deferred |
| Changelog tooling | Manual `CHANGELOG.md` for v1; changie deferred |
| Self-update command | Deferred to follow-up RFC |

---

## Design

### 1. Release Automation — cargo-dist

[cargo-dist](https://github.com/axodotdev/cargo-dist) is the de-facto standard for shipping Rust CLI binaries across platforms. It:

- Cross-compiles for all targets in a single `cargo dist build` invocation.
- Generates a GitHub Release with per-platform archives and a `dist-manifest.json`.
- Generates and maintains the install shell scripts (`install.sh` / `install.ps1`) automatically.
- Integrates natively with Homebrew tap publishing.

**Initialization** (one-time, produces committed config):

```bash
cargo install cargo-dist
cargo dist init
```

`cargo dist init` is interactive — select: installers → `shell`, `powershell`, `homebrew`; tap → `relateby/homebrew-tap`. This adds a `[workspace.metadata.dist]` section to `Cargo.toml` and generates `.github/workflows/release.yml`.

**Resulting `Cargo.toml` additions:**

```toml
[workspace.metadata.dist]
cargo-dist-version = "0.x.y"
ci = "github"
installers = ["shell", "powershell", "homebrew", "npm"]
tap = "relateby/homebrew-tap"
publish-jobs = ["homebrew"]
targets = [
  "x86_64-unknown-linux-gnu",
  "aarch64-unknown-linux-gnu",
  "x86_64-apple-darwin",
  "aarch64-apple-darwin",
  "x86_64-pc-windows-msvc",
]
install-updater = false
```

**Build matrix:**

| Target | OS | Arch |
|--------|----|------|
| `x86_64-unknown-linux-gnu` | Linux | x86_64 |
| `aarch64-unknown-linux-gnu` | Linux | ARM64 |
| `x86_64-apple-darwin` | macOS | Intel |
| `aarch64-apple-darwin` | macOS | Apple Silicon |
| `x86_64-pc-windows-msvc` | Windows | x86_64 |

Archive format: `.tar.gz` (Linux/macOS), `.zip` (Windows). Each archive contains the `relate` binary and `README.md`. Checksums: `sha256` file published alongside every release.

### 2. Homebrew Tap

A dedicated public tap repository at `relateby/homebrew-tap` hosts the `Formula/relate.rb` formula. cargo-dist generates and pushes an updated formula on every stable (`vX.Y.Z`) release; prereleases skip the formula push automatically.

Users install with:

```bash
brew install relateby/tap/relate
```

**One-time setup:**

1. Create the public repo `relateby/homebrew-tap` with a `Formula/` directory and a stub `README.md`.
2. Create a GitHub fine-grained Personal Access Token scoped to **only** `relateby/homebrew-tap`, with **Contents: Read and Write** permission. (GitHub Settings → Developer Settings → Personal access tokens → Fine-grained tokens → New token.)
3. Add the token as a secret named `HOMEBREW_TAP_TOKEN` in `relateby/relate-cli` (repo Settings → Secrets and variables → Actions).
4. The generated `release.yml` will already reference this secret — no further changes needed.

The formula `Formula/relate.rb` will be created automatically on the first stable release. You do not need to write it by hand.

### 3. npm Wrapper Packages

Following neo4j-cli's pattern, npm distribution uses a **thin shim + platform packages** approach:

- One wrapper `@relateby/cli` with `optionalDependencies` pointing to five platform packages.
- Five packages `@relateby/cli-linux-x64`, `@relateby/cli-linux-arm64`, `@relateby/cli-darwin-x64`, `@relateby/cli-darwin-arm64`, `@relateby/cli-win32-x64`, each containing the prebuilt binary and a `package.json` with `os`/`cpu` constraints.
- A Node.js shim `bin/relate.js` that resolves the installed platform binary via `require.resolve()` and `child_process.spawn()`.

**Why not a postinstall download script?** Platform-specific `optionalDependencies` let npm itself perform platform selection with no scripts running at install time. This works in `--ignore-scripts` CI environments, pnpm, Bun, and restricted corporate registries.

The packages live in `distribution/npm/` and are published by a `publish-npm.yml` workflow triggered on `release` events.

Users install with:

```bash
npm i -g @relateby/cli         # stable
npm i -g @relateby/cli@next    # prerelease
```

**One-time setup:** Publish an empty `0.0.0` placeholder for each package name to claim them on npmjs.com, then publish properly on first real release.

### 4. Shell Install Script

cargo-dist generates `install.sh` and `install.ps1` automatically. These scripts:

- Detect OS and architecture via `uname`.
- Fetch the latest release version from the GitHub Releases API.
- Download the matching archive from GitHub Releases.
- Verify the sha256 checksum.
- Extract the binary to `~/.local/bin` (macOS/Linux) or `%LOCALAPPDATA%\Programs\relate` (Windows).
- Alert the user if the install directory is not in `$PATH`.

The canonical install command will be:

```bash
curl -sSfL https://cli.relateby.dev/install.sh | bash
```

The scripts are committed alongside the website and served from `gh-pages`.

### 5. Project Website (gh-pages)

**Domain:** `cli.relateby.dev` (registered on GoDaddy). DNS CNAME points to `relateby.github.io`.

**Hosting:** Static files in a `gh-pages` branch of `relateby/relate-cli`, with a `CNAME` file containing `cli.relateby.dev`. GitHub Pages serves it with a free TLS certificate via Let's Encrypt.

**Technology:** Vanilla HTML + CSS + JavaScript, no framework. Self-hosted fonts (Public Sans for prose, Fira Code for terminal snippets). CSS custom properties for light/dark theming — same design language as neo4j.sh.

**Content sections:**

| Section | Content |
|---------|---------|
| **Hero** | Tagline, OS-detecting install tabs (macOS · Linux · Windows · Homebrew · npm · cargo), one-click copy |
| **Quickstart** | Four steps: install → `relate lint` → connect to Neo4j → run a query |
| **Commands** | Tabbed examples for `lint`, `parse`, `query`, `query --apply`, `mcp` |
| **Agent Skills** | How to install the skill for Claude Code / Cursor / Copilot |
| **Install** | Full install matrix with all distribution channels |

**Sync with releases:** A `update-website.yml` workflow runs on the `release` event, copies `install.sh` / `install.ps1` from the release assets into the `gh-pages` branch, and updates the version badge in `index.html`.

### 6. GitHub Actions Workflows

| Workflow | Trigger | Action |
|----------|---------|--------|
| `ci.yml` | push / PR | `cargo fmt --check`, `clippy -D warnings`, `cargo test` |
| `release.yml` | push of `v*` tag | `cargo dist build`, create GitHub Release, push Homebrew formula, `cargo publish` to crates.io |
| `publish-npm.yml` | GitHub Release published | Build npm packages, `npm publish` all six packages |
| `update-website.yml` | GitHub Release published | Copy install scripts to `gh-pages`, update version in `index.html` |

**Release trigger:** A version tag `vX.Y.Z` on `main`. The version in `Cargo.toml` must match the tag. A pre-release tag `vX.Y.Z-beta.N` publishes to GitHub Releases and npm `@next` but skips the Homebrew formula push.

**Secrets required in `relateby/relate-cli`:**

| Secret name | Used by | Purpose |
|-------------|---------|---------|
| `HOMEBREW_TAP_TOKEN` | `release.yml` | Push formula to `relateby/homebrew-tap` |
| `CARGO_REGISTRY_TOKEN` | `release.yml` | Publish to crates.io |
| `NPM_TOKEN` | `publish-npm.yml` | Publish to npmjs.com |
| ~~`APPLE_CERTIFICATE`~~ | — | Not required (see §Appendix A) |

### 7. `cargo install` (crates.io)

`cargo publish` runs as a final step in `release.yml` after the GitHub Release is created. The `Cargo.toml` metadata is already complete. This gives Rust developers a `cargo install relate` path at no extra maintenance cost.

### 8. Repository Layout Changes

```
distribution/
├── npm/
│   ├── cli/
│   │   ├── bin/relate.js           # Node.js shim
│   │   └── package.json.tmpl       # wrapper with optionalDependencies
│   ├── cli-platform/
│   │   └── package.json.tmpl       # per-platform template
│   └── platforms.tsv               # OS/arch/npm-name mapping (source of truth)
website/
├── index.html
├── install.sh                      # generated by cargo-dist, committed here
├── install.ps1
├── fonts/
│   ├── public-sans.woff2
│   └── fira-code.woff2
├── CNAME                           # cli.relateby.dev
├── robots.txt
└── sitemap.xml
```

The `website/` directory is maintained in `main` and deployed to `gh-pages` by `update-website.yml`. This keeps the source of truth in `main` and avoids the `gh-pages` orphan branch diverging silently.

---

## Appendix A: macOS and Gatekeeper

macOS Gatekeeper only quarantines files that carry the `com.apple.quarantine` extended attribute. **This attribute is set by browsers (Safari, Chrome, Firefox) when files are downloaded, but not by terminal tools.** As a result:

| Install path | Quarantine bit set? | Gatekeeper blocks? |
|---|---|---|
| `curl \| bash` | No | No |
| `brew install` | No | No |
| `npm i -g` | No | No |
| `cargo install` | No (compiled locally) | No |
| Manual `.tar.gz` download in browser → run | Yes | **Yes** |

The manual browser-download path is the only one that triggers a Gatekeeper warning ("Apple cannot verify that this app is free from malware"). The fix is a one-time command:

```bash
xattr -d com.apple.quarantine /usr/local/bin/relate
```

This workaround should be documented prominently in the README and on the website under the manual install instructions. It is widely understood by developers and is the standard practice for open-source CLI tools that do not pay for an Apple Developer account.

**Decision:** Skip macOS signing for now. Revisit if user feedback indicates the Gatekeeper friction is meaningfully reducing adoption.

---

## Appendix B: Homebrew Tap Token Setup

### B.1 Create the tap repository

Create `relateby/homebrew-tap` as a **public** repository. Add a top-level `Formula/` directory (GitHub requires at least one file — add a `README.md`). This is the only manual step; cargo-dist writes and maintains the formula file.

### B.2 Create a fine-grained PAT

1. On GitHub, go to **Settings → Developer Settings → Personal access tokens → Fine-grained tokens → Generate new token**.
2. **Token name:** `relate-homebrew-tap`
3. **Expiration:** 1 year (set a calendar reminder to rotate it)
4. **Resource owner:** `relateby` (the org)
5. **Repository access:** Only select repositories → `relateby/homebrew-tap`
6. **Permissions:** Repository permissions → Contents → **Read and write**
7. Click **Generate token** and copy it immediately.

### B.3 Store the PAT as a secret

In `relateby/relate-cli` → Settings → Secrets and variables → Actions → New repository secret:

- **Name:** `HOMEBREW_TAP_TOKEN`
- **Value:** the token from B.2

The generated `release.yml` workflow already uses `HOMEBREW_TAP_TOKEN` — no further edits needed.

---

## Unresolved Questions

1. **DNS propagation.** After pointing the GoDaddy CNAME for `cli.relateby.dev` to `relateby.github.io`, enable **Enforce HTTPS** in the GitHub Pages settings (available once the Let's Encrypt certificate is issued, typically within minutes).

2. **Windows signing.** An Authenticode certificate reduces SmartScreen warnings. Cost is ~$300–$500/year from a CA like DigiCert or Sectigo. Deferred to a follow-up RFC.

3. **Changie / changelog tooling.** neo4j-cli uses [changie](https://changie.dev/) for structured per-entry changelogs that cargo-dist embeds in release notes. A manual `CHANGELOG.md` is sufficient for early releases. Adopt changie when the release cadence increases.

4. **Self-update command.** Implementing `relate update` (fetching and replacing the binary via the GitHub Releases API) would improve the experience for `curl | bash` installs. Deferred to a follow-up RFC.
