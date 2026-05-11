# Research: Packaging and Distribution

## Decision 1: Release automation tool

**Decision**: cargo-dist (`[package.metadata.dist]` in `Cargo.toml`)

**Rationale**: cargo-dist is the de-facto standard for Rust CLI distribution. It generates the GitHub Actions release workflow, cross-compiled archives for all targets, install scripts (`install.sh` / `install.ps1`), and pushes Homebrew formula updates in a single integrated pipeline. For a single-crate project with no `[workspace]` table, the config goes under `[package.metadata.dist]` (not `[workspace.metadata.dist]`).

**Alternatives considered**:
- GoReleaser: Go-centric, requires extra plugins for Rust; not native to the Cargo ecosystem.
- Manual cross-compilation + release scripts: high maintenance, error-prone.

**Key config** (added to `Cargo.toml`):
```toml
[package.metadata.dist]
cargo-dist-version = "<current>"   # pinned by cargo dist init
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
create-release = true
install-updater = false
npm-scope = "@relateby"
```

**Bootstrap command**: `cargo install cargo-dist && cargo dist init` (interactive; writes the above automatically).

---

## Decision 2: npm binary distribution pattern

**Decision**: optionalDependencies + Node.js shim (no postinstall scripts)

**Rationale**: The `optionalDependencies` pattern (used by esbuild, @biomejs/biome) lets npm resolve the correct platform binary during dependency resolution — no scripts execute at install time. This is the only approach that works in `--ignore-scripts` CI environments and with pnpm/Bun.

**Alternatives considered**:
- `postinstall` download script: breaks in airgapped environments, restricted CI, and when the GitHub Releases URL is temporarily unavailable.
- Single package with all binaries: bloated (5× archive size for every install).

**Package structure**:

Wrapper `@relateby/cli/package.json`:
```json
{
  "name": "@relateby/cli",
  "version": "0.1.0",
  "bin": { "relate": "bin/relate.js" },
  "optionalDependencies": {
    "@relateby/cli-linux-x64":    "0.1.0",
    "@relateby/cli-linux-arm64":  "0.1.0",
    "@relateby/cli-darwin-x64":   "0.1.0",
    "@relateby/cli-darwin-arm64": "0.1.0",
    "@relateby/cli-win32-x64":    "0.1.0"
  }
}
```

Platform package `@relateby/cli-linux-x64/package.json`:
```json
{
  "name": "@relateby/cli-linux-x64",
  "version": "0.1.0",
  "os": ["linux"],
  "cpu": ["x64"],
  "files": ["bin/relate"]
}
```

**`os`/`cpu` mapping**:

| Target triple | `os` | `cpu` |
|---|---|---|
| `x86_64-unknown-linux-gnu` | `linux` | `x64` |
| `aarch64-unknown-linux-gnu` | `linux` | `arm64` |
| `x86_64-apple-darwin` | `darwin` | `x64` |
| `aarch64-apple-darwin` | `darwin` | `arm64` |
| `x86_64-pc-windows-msvc` | `win32` | `x64` |

**Shim `bin/relate.js`**:
```js
#!/usr/bin/env node
"use strict";
const { spawnSync } = require("child_process");
const PKGS = {
  "linux-x64":    "@relateby/cli-linux-x64",
  "linux-arm64":  "@relateby/cli-linux-arm64",
  "darwin-x64":   "@relateby/cli-darwin-x64",
  "darwin-arm64": "@relateby/cli-darwin-arm64",
  "win32-x64":    "@relateby/cli-win32-x64",
};
const key = `${process.platform}-${process.arch}`;
const pkg = PKGS[key];
if (!pkg) { console.error(`relate: unsupported platform ${key}`); process.exit(1); }
let bin;
try { bin = require.resolve(`${pkg}/bin/relate`); }
catch { console.error(`relate: platform package "${pkg}" not installed.\nTry: npm install ${pkg}`); process.exit(1); }
const r = spawnSync(bin, process.argv.slice(2), { stdio: "inherit" });
process.exit(r.status ?? 1);
```

---

## Decision 3: Website hosting

**Decision**: GitHub Pages from `gh-pages` branch of `relateby/relate-cli`, custom domain `cli.relateby.dev`

**Rationale**: Free TLS via Let's Encrypt, zero infrastructure cost, automatic deployment from the same repository. No Cloudflare required for a subdomain CNAME (unlike apex domain which needs A records).

**Alternatives considered**:
- Cloudflare Pages: adds complexity; only needed if edge caching or Workers are required.
- Vercel/Netlify: free tier but adds third-party dependency.

---

## Decision 4: GoDaddy CNAME → GitHub Pages

**Decision**: Single CNAME record; `CNAME` file in `gh-pages` root.

**DNS record** (add in GoDaddy DNS Manager for `relateby.dev`):

| Field | Value |
|---|---|
| Type | CNAME |
| Name / Host | `cli` |
| Points To | `relateby.github.io` |
| TTL | 600s (1 hour) |

**`CNAME` file content** (in `gh-pages` branch root):
```
cli.relateby.dev
```

**GitHub Pages settings**: Settings → Pages → Custom domain: `cli.relateby.dev` → Save → tick Enforce HTTPS (available ~15 min after DNS propagates).

**Propagation**: typically 5–30 min for GoDaddy subdomain CNAMEs; verify with `dig cli.relateby.dev CNAME +short` (expect `relateby.github.io.`).

**Gotcha**: GoDaddy pre-populates a "parked" CNAME that conflicts — delete it first.

---

## Decision 5: macOS Gatekeeper

**Decision**: No signing; document `xattr` workaround for manual downloads.

**Rationale**: All primary install paths (`curl | bash`, Homebrew, npm) do not set `com.apple.quarantine`. Only browser-downloaded archives trigger Gatekeeper. The workaround (`xattr -d com.apple.quarantine /path/to/relate`) is standard practice for open-source CLIs and well understood by the target audience.

**Cost avoided**: $99/year Apple Developer account, plus workflow complexity for notarization.
