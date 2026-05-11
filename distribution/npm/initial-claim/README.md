# npm Initial Claim

These directories contain the minimal `package.json` files needed to **claim the six
npm package names** before the CI release pipeline runs for the first time.

## Why this exists

npm's trusted publisher (OIDC) workflow requires the package to already exist under
your npm account before you can configure a trusted publisher for it. These placeholder
`0.0.0` packages claim the names without shipping a real binary.

## One-time publish procedure

You must be logged in to npm as an `@relateby` org member:

```bash
npm login   # or: npm login --auth-type=legacy
```

Then publish each package:

```bash
cd distribution/npm/initial-claim

npm publish @relateby/cli          --access public
npm publish @relateby/cli-linux-x64   --access public
npm publish @relateby/cli-linux-arm64 --access public
npm publish @relateby/cli-darwin-x64  --access public
npm publish @relateby/cli-darwin-arm64 --access public
npm publish @relateby/cli-win32-x64   --access public
```

## After publishing

1. Go to npmjs.com → each package → **Settings → Trusted Publishers**
2. Add a trusted publisher for each: org = `relateby`, repo = `relate-cli`, workflow = `publish-npm.yml`
3. The CI pipeline handles all future publishes automatically — no stored token needed.

## Cleanup

Once the first real release (v0.1.0) has been published by CI and trusted publishers
are configured, this entire `initial-claim/` directory can be deleted.
