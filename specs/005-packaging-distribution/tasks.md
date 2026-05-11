# Tasks: Packaging and Distribution

**Input**: Design documents from `specs/005-packaging-distribution/`
**Prerequisites**: plan.md ✅, spec.md ✅, research.md ✅, data-model.md ✅, contracts/ ✅, quickstart.md ✅

**Organization**: Tasks are grouped by user story to enable independent implementation and testing of each story.
**Tests**: No test tasks generated — no test framework is applicable to release pipeline and static website deliverables; validation is done by smoke-testing install paths.

## Format: `[ID] [P?] [Story] Description`

- **[P]**: Can run in parallel (different files, no dependencies)
- **[Story]**: Which user story this task belongs to (US1–US5)
- Exact file paths included in all descriptions

---

## Phase 1: Setup (Shared Infrastructure)

**Purpose**: Initialize release tooling, create external repos, and scaffold directory structure.

- [ ] T001 Run `cargo install cargo-dist && cargo dist init` (select: shell, powershell, homebrew, npm installers; tap = `relateby/homebrew-tap`; npm-scope = `@relateby`; 5 targets); commit resulting changes to `Cargo.toml` and generated `.github/workflows/release.yml`
- [ ] T002 Create public GitHub repo `relateby/homebrew-tap` with a `Formula/` directory and `Formula/README.md` stub file
- [ ] T003 [P] Create `CHANGELOG.md` at repo root with a `## Unreleased` section and standard keep-a-changelog header
- [ ] T004 [P] Create `distribution/npm/` directory structure: `platforms.tsv` (empty), `cli/bin/` (empty), `cli-platform/` (empty)
- [ ] T005 [P] Create `website/` directory structure: `fonts/`, `CNAME`, `robots.txt`, `sitemap.xml` (all as stubs)

---

## Phase 2: Foundational (Blocking Prerequisites)

**Purpose**: External accounts, credentials, and DNS wired up — required before any release or website can go live.

**⚠️ CRITICAL**: No user story validation can be completed until this phase is done.

- [ ] T006 Create fine-grained GitHub PAT scoped only to `relateby/homebrew-tap` with Contents: Read and Write permission; store as `HOMEBREW_TAP_TOKEN` secret in `relateby/relate-cli` Actions secrets
- [ ] T007 [P] Create crates.io API token (crates.io → Account Settings → API Tokens); store as `CARGO_REGISTRY_TOKEN` secret in `relateby/relate-cli` Actions secrets
- [ ] T008 [P] Create `@relateby` npm organization at npmjs.com; generate an Automation token; store as `NPM_TOKEN` secret in `relateby/relate-cli` Actions secrets
- [ ] T009 Add CNAME record in GoDaddy DNS Manager for `relateby.dev`: Type = CNAME, Name/Host = `cli`, Points To = `relateby.github.io`, TTL = 600; remove any pre-existing conflicting "parked" CNAME first
- [ ] T010 [P] Publish six `0.0.0` placeholder packages to claim npm names: `@relateby/cli`, `@relateby/cli-linux-x64`, `@relateby/cli-linux-arm64`, `@relateby/cli-darwin-x64`, `@relateby/cli-darwin-arm64`, `@relateby/cli-win32-x64`

**Checkpoint**: Secrets exist in repo settings, DNS record is live (`dig cli.relateby.dev CNAME +short` returns `relateby.github.io.`), npm names are claimed.

---

## Phase 3: User Story 5 + User Story 1 (Priority: P1) 🎯 MVP

**US5 Goal**: Pushing a `vX.Y.Z` tag to `main` publishes all artifacts automatically with no manual steps.
**US1 Goal**: `curl -sSfL https://cli.relateby.dev/install.sh | bash` installs a working `relate` binary on macOS and Linux.

**Why combined**: US1 is only verifiable after US5's pipeline runs. They are P1 co-dependencies.

**Independent Test (US5)**: Push `v0.1.0` tag; all pipeline jobs complete green with no manual intervention; GitHub Release has 5 archives + checksums.

**Independent Test (US1)**: Run `curl -sSfL https://cli.relateby.dev/install.sh | bash` on fresh macOS (Apple Silicon) and Ubuntu; `relate --version` succeeds immediately after.

### Implementation

- [ ] T011 [US5] Add `cargo publish` step to `.github/workflows/release.yml` (after GitHub Release creation; gated on `!contains(github.ref, '-')` to skip prereleases)
- [ ] T012 [US5] Push prerelease tag `v0.1.0-beta.1`; verify: GitHub Release is created and marked prerelease; `Formula/relate.rb` is NOT present in `relateby/homebrew-tap`; crates.io is NOT updated
- [ ] T013 [US5] Push stable tag `v0.1.0`; verify: GitHub Release contains 5 `.tar.gz`/`.zip` archives, `relate-checksums.txt`, `dist-manifest.json`, `install.sh`, and `install.ps1`
- [ ] T014 [US5] Verify `Formula/relate.rb` is present and correct in `relateby/homebrew-tap` after the stable release pipeline completes
- [ ] T015 [US5] [P] Verify `relate` version `0.1.0` appears on crates.io after the stable release pipeline completes
- [ ] T016 [US1] Write `update-website.yml` initial version: trigger on `release` event (type: `published`, stable only); copies `install.sh` and `install.ps1` from release assets to `website/` in the `gh-pages` branch; commits and pushes — file: `.github/workflows/update-website.yml`
- [ ] T017 [US1] Verify `curl -sSfL https://cli.relateby.dev/install.sh | bash` succeeds on macOS Apple Silicon (aarch64); confirm `relate --version` prints `0.1.0`
- [ ] T018 [US1] [P] Verify `curl -sSfL https://cli.relateby.dev/install.sh | bash` succeeds on Ubuntu x86_64; confirm `relate --version` prints `0.1.0`
- [ ] T019 [US1] Verify install script aborts with a clear error message when the downloaded archive sha256 does not match `relate-checksums.txt`
- [ ] T020 [US1] Verify install script prints PATH setup instructions when `~/.local/bin` is not in `$PATH`

**Checkpoint**: Tagging triggers a full release; `curl | bash` installs a working binary on macOS and Linux; pipeline failures surface clearly.

---

## Phase 4: User Story 2 — Homebrew install (Priority: P2)

**Goal**: `brew install relateby/tap/relate` installs `relate` on macOS and upgrades cleanly on subsequent releases.

**Independent Test**: Run `brew install relateby/tap/relate` on a fresh macOS machine with no prior `relate` install; `relate --version` succeeds; `brew upgrade relate` works after pushing a second tag.

### Implementation

- [ ] T021 [US2] Verify `brew install relateby/tap/relate` succeeds on macOS and installs the correct version from the `v0.1.0` release
- [ ] T022 [US2] Push `v0.1.1` stable tag; verify `brew upgrade relate` updates the binary to `0.1.1`
- [ ] T023 [US2] Push `v0.1.2-rc.1` prerelease tag; verify Homebrew formula is NOT updated (still at `0.1.1`)

**Checkpoint**: Homebrew tap self-maintains on every stable release; prereleases are ignored.

---

## Phase 5: User Story 4 — Project website (Priority: P2)

**Goal**: `cli.relateby.dev` is live, OS-aware, and stays in sync with every stable release.

**Independent Test**: Open `https://cli.relateby.dev` in Safari on macOS — macOS tab is pre-selected; copy button works; mobile 375px viewport is readable; version badge matches latest GitHub Release.

### Implementation

- [ ] T024 [US4] Write `website/index.html` hero section: heading, tagline, OS-detecting install tab bar (macOS · Linux · Windows · Homebrew · npm · cargo), one command per tab, copy-to-clipboard buttons — file: `website/index.html`
- [ ] T025 [US4] Add OS detection JS to `website/index.html`: reads `navigator.platform` / `navigator.userAgent`; pre-selects matching tab on page load; falls back to macOS
- [ ] T026 [US4] Add Quickstart section to `website/index.html`: 4 numbered steps (install → `relate lint` → connect to Neo4j → run a query)
- [ ] T027 [US4] [P] Add Commands section to `website/index.html`: tabbed examples for `lint`, `parse`, `query`, `query --apply`, `mcp` with syntax-highlighted code blocks
- [ ] T028 [US4] [P] Add Agent Skills section to `website/index.html`: how to install the skill for Claude Code / Cursor / Copilot with copy-able `npx skills` command
- [ ] T029 [US4] [P] Add Install matrix table to `website/index.html`: all 4 channels × platforms with one-liner commands
- [ ] T030 [US4] Add light/dark theme toggle to `website/index.html`: CSS custom properties, `data-theme` attribute, toggle button; theme persists via `localStorage`
- [ ] T031 [US4] [P] Add `<span id="current-version">` version badge to `website/index.html` hero section (populated by `update-website.yml`)
- [ ] T032 [US4] [P] Download and add self-hosted fonts to `website/fonts/`: Public Sans (Regular, Medium, SemiBold `.woff2`) and Fira Code (Regular `.woff2`)
- [ ] T033 [US4] Write CSS for `website/index.html`: CSS custom properties for light/dark theme, responsive grid, terminal code block styling, tab components — all inline or in a `<style>` block
- [ ] T034 [US4] Write `website/CNAME` containing exactly `cli.relateby.dev`
- [ ] T035 [US4] [P] Write `website/robots.txt` (allow all) and `website/sitemap.xml` (single URL: `https://cli.relateby.dev/`)
- [ ] T036 [US4] Extend `.github/workflows/update-website.yml` to also update the `<span id="current-version">` text in `website/index.html` with the release tag version before pushing to `gh-pages`
- [ ] T037 [US4] Enable GitHub Pages on `relateby/relate-cli`: Source = `gh-pages` branch; Custom domain = `cli.relateby.dev`; enable Enforce HTTPS after DNS check passes (~15–30 min after DNS propagation)
- [ ] T038 [US4] Verify `https://cli.relateby.dev` loads; macOS tab is auto-selected on macOS; copy button pastes correct command; version badge shows current release; 375px mobile viewport is readable in both light and dark modes

**Checkpoint**: Website live at `https://cli.relateby.dev`; version badge and install scripts update automatically within 5 minutes of a stable release.

---

## Phase 6: User Story 3 — npm install (Priority: P3)

**Goal**: `npm i -g @relateby/cli` installs the correct binary on all 5 platforms without any postinstall scripts.

**Independent Test**: Run `npm i -g @relateby/cli` on macOS, Linux x86_64, and Linux arm64 with `--ignore-scripts`; `relate --version` succeeds on all; only one platform binary package is downloaded per install.

### Implementation

- [ ] T039 [US3] Write `distribution/npm/platforms.tsv`: 5 rows mapping target triple → npm package name → os value → cpu value
- [ ] T040 [US3] Write `distribution/npm/cli-platform/package.json.tmpl`: platform package template with `%%NAME%%`, `%%VERSION%%`, `%%OS%%`, `%%CPU%%` substitution markers — file: `distribution/npm/cli-platform/package.json.tmpl`
- [ ] T041 [US3] Write `distribution/npm/cli/package.json.tmpl`: wrapper package with `"name": "@relateby/cli"`, `"bin": {"relate": "bin/relate.js"}`, and `optionalDependencies` block referencing all 5 platform packages — file: `distribution/npm/cli/package.json.tmpl`
- [ ] T042 [US3] Write `distribution/npm/cli/bin/relate.js`: Node.js shim using `require.resolve()` to locate the platform binary and `child_process.spawnSync()` to exec it; handles unsupported platform and missing optional package with clear error messages — file: `distribution/npm/cli/bin/relate.js`
- [ ] T043 [US3] Write `.github/workflows/publish-npm.yml`: trigger on `release` event (type: `published`); download all 5 platform archives from GitHub Release; extract binaries; stamp `package.json.tmpl` files with release version; `npm publish` platform packages first then wrapper; use `--tag latest` for stable releases and `--tag next` for prereleases — file: `.github/workflows/publish-npm.yml`
- [ ] T044 [US3] Trigger a release and verify `npm i -g @relateby/cli` installs correctly on macOS (darwin-arm64); confirm only `@relateby/cli-darwin-arm64` is downloaded
- [ ] T045 [US3] [P] Verify `npm i -g @relateby/cli` installs correctly on Linux x86_64; confirm only `@relateby/cli-linux-x64` is downloaded
- [ ] T046 [US3] [P] Verify `npm i -g @relateby/cli --ignore-scripts` succeeds on macOS; confirm no postinstall script errors
- [ ] T047 [US3] Verify `npm i -g @relateby/cli@next` installs the latest prerelease; confirm `relate --version` shows the prerelease version

**Checkpoint**: `npm i -g @relateby/cli` works on all tested platforms; no scripts execute at install time; `@next` tag tracks prereleases.

---

## Phase 7: Polish & Cross-Cutting Concerns

**Purpose**: Documentation, discoverability, and end-to-end validation.

- [ ] T048 [P] Update `README.md` install section: add all four install channels with exact commands (`curl | bash`, `brew install`, `npm i -g`, `cargo install`) — file: `README.md`
- [ ] T049 [P] Add macOS Gatekeeper workaround to `README.md` under the manual download note: `xattr -d com.apple.quarantine /path/to/relate` — file: `README.md`
- [ ] T050 [P] Update `skills/relate/SKILL.md` install section to include the website URL `https://cli.relateby.dev` — file: `skills/relate/SKILL.md`
- [ ] T051 Run end-to-end validation per `specs/005-packaging-distribution/quickstart.md`: fresh install via all four channels; verify `relate --version` matches latest release on each

**Checkpoint**: All install paths documented; website URL in skill docs; end-to-end validation passes on all channels.

---

## Dependencies & Execution Order

### Phase Dependencies

- **Setup (Phase 1)**: No dependencies — start immediately
- **Foundational (Phase 2)**: Depends on Phase 1 — **BLOCKS all user story validation**
- **US5+US1 (Phase 3)**: Depends on Phase 2 — the entire distribution pipeline
- **US2 (Phase 4)**: Depends on Phase 3 (needs at least one stable release to test `brew upgrade`)
- **US4 (Phase 5)**: Depends on Phase 3 (website serves install scripts from releases)
- **US3 (Phase 6)**: Depends on Phase 3 (npm packages bundle binaries from releases); can run in parallel with US2 and US4
- **Polish (Phase 7)**: Depends on all user stories being complete

### User Story Dependencies

- **US5+US1 (P1)**: Depends on Foundational only — no story dependencies
- **US2 (P2)**: Depends on US5 pipeline being proven (needs a second release to test upgrade)
- **US4 (P2)**: Depends on US5 pipeline (install scripts served from releases); can run in parallel with US2
- **US3 (P3)**: Depends on US5 pipeline; can run in parallel with US2 and US4

### Parallel Opportunities

Within Phase 3: T015 (crates.io check) in parallel with T016 (website workflow)
Within Phase 5: T027, T028, T029, T031, T032, T035 all touch different sections of `index.html` or separate files
Within Phase 6: T044, T045, T046 (npm install verification on different platforms) all parallel

---

## Parallel Example: Phase 5 (Website)

```
# Launch together (different sections of index.html or different files):
Task T027: "Add Commands section to website/index.html"
Task T028: "Add Agent Skills section to website/index.html"
Task T029: "Add Install matrix table to website/index.html"
Task T031: "Add version badge span to website/index.html"
Task T032: "Download and add self-hosted fonts to website/fonts/"

# Then sequentially:
Task T033: "Write CSS for all sections"
Task T034: "Write website/CNAME"
```

---

## Implementation Strategy

### MVP First (Phase 1 + 2 + 3 only)

1. Complete Phase 1: Setup (T001–T005)
2. Complete Phase 2: Foundational (T006–T010) — REQUIRED before any release
3. Complete Phase 3: US5+US1 (T011–T020)
4. **STOP and VALIDATE**: `curl | bash` installs on macOS and Linux; pipeline is fully automated
5. Ship — all other channels are additive

### Incremental Delivery

1. Phase 1+2 → infrastructure ready
2. Phase 3 → `curl | bash`, Homebrew, crates.io working; **MVP shippable**
3. Phase 5 → website live at `cli.relateby.dev`
4. Phase 4 → confirm Homebrew upgrades work across releases
5. Phase 6 → npm distribution
6. Phase 7 → docs complete

### Total Task Count

- Setup: 5 tasks
- Foundational: 5 tasks
- US5+US1: 10 tasks
- US2: 3 tasks
- US4: 15 tasks
- US3: 9 tasks
- Polish: 4 tasks

**Total: 51 tasks** | **Parallel opportunities: 18 tasks marked [P]**
