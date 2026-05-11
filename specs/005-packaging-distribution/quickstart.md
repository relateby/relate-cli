# Quickstart: Doing a Release

This guide walks a maintainer through every step of releasing a new version of `relate`.

## Prerequisites (one-time setup)

Complete these steps before the first release. Each only needs to be done once.

### 1. Initialize cargo-dist

```bash
cargo install cargo-dist
cargo dist init   # interactive: choose shell, powershell, homebrew, npm installers
                  # tap: relateby/homebrew-tap
                  # npm scope: @relateby
```

Commit the changes to `Cargo.toml` and the generated `.github/workflows/release.yml`.

### 2. Create the Homebrew tap repo

1. Create `relateby/homebrew-tap` as a public repo with a `Formula/` directory and a stub `README.md`.
2. Create a fine-grained GitHub PAT (Settings → Developer Settings → Fine-grained tokens):
   - Scope: only `relateby/homebrew-tap`
   - Permission: Contents → Read and write
3. Add it as `HOMEBREW_TAP_TOKEN` in `relateby/relate-cli` Actions secrets.

### 3. Set up npm

1. Create an npm org account for `@relateby` at npmjs.com.
2. Generate an npm automation token (not a publish token — automation tokens work in CI).
3. Add it as `NPM_TOKEN` in `relateby/relate-cli` Actions secrets.
4. Claim the package names by publishing empty `0.0.0` placeholders for all 6 packages.

### 4. Set up crates.io

1. Create a crates.io account and verify email.
2. Generate an API token (Account Settings → API Tokens).
3. Add it as `CARGO_REGISTRY_TOKEN` in `relateby/relate-cli` Actions secrets.

### 5. Set up the website and DNS

1. Copy `website/` into the `gh-pages` branch (or let `update-website.yml` do it on first release).
2. In GoDaddy DNS Manager for `relateby.dev`, add:
   - Type: CNAME, Name: `cli`, Points to: `relateby.github.io`, TTL: 600
   - Delete any conflicting "parked" CNAME first.
3. In GitHub Pages settings (relateby/relate-cli → Settings → Pages):
   - Custom domain: `cli.relateby.dev` → Save
   - After DNS propagates (~15–30 min): tick Enforce HTTPS

---

## Normal Release (every time)

### Step 1: Bump the version

```bash
# Edit Cargo.toml: version = "X.Y.Z"
cargo build    # updates Cargo.lock
git add Cargo.toml Cargo.lock
git commit -m "chore: bump version to X.Y.Z"
```

Update `CHANGELOG.md` with the release notes.

### Step 2: Tag and push

```bash
git tag vX.Y.Z
git push origin main --tags
```

That's it. The pipeline does the rest.

### Step 3: Watch the pipeline

In GitHub Actions, the `release.yml` workflow will:
1. Build binaries for all 5 targets
2. Create a GitHub Release with archives + checksums
3. Push the Homebrew formula to `relateby/homebrew-tap`
4. Trigger `publish-npm.yml` → publish all 6 npm packages
5. Trigger `update-website.yml` → update `gh-pages` with new install scripts + version

Total pipeline time: ~10–15 minutes.

---

## Prerelease

```bash
# In Cargo.toml: version = "X.Y.Z-beta.1"
git add Cargo.toml Cargo.lock && git commit -m "chore: bump to X.Y.Z-beta.1"
git tag vX.Y.Z-beta.1
git push origin main --tags
```

Prerelease behavior:
- GitHub Release is created (marked prerelease)
- npm `@next` tag is updated
- Homebrew formula is **not** updated
- crates.io is **not** published

---

## Verifying a Release

```bash
# Install fresh and verify
curl -sSfL https://cli.relateby.dev/install.sh | bash
relate --version   # should print vX.Y.Z

brew upgrade relate && relate --version

npm i -g @relateby/cli@latest && relate --version

cargo install relate --force && relate --version
```
