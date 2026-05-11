#!/usr/bin/env node
"use strict";

const { spawnSync } = require("child_process");

const PLATFORM_PACKAGES = {
  "linux-x64":    "@relateby/cli-linux-x64",
  "linux-arm64":  "@relateby/cli-linux-arm64",
  "darwin-x64":   "@relateby/cli-darwin-x64",
  "darwin-arm64": "@relateby/cli-darwin-arm64",
  "win32-x64":    "@relateby/cli-win32-x64",
};

const key = `${process.platform}-${process.arch}`;
const pkg = PLATFORM_PACKAGES[key];

if (!pkg) {
  process.stderr.write(
    `relate: unsupported platform: ${process.platform}/${process.arch}\n` +
    `Supported: ${Object.keys(PLATFORM_PACKAGES).join(", ")}\n`
  );
  process.exit(1);
}

const binaryName = process.platform === "win32" ? "relate.exe" : "relate";

let binaryPath;
try {
  binaryPath = require.resolve(`${pkg}/bin/${binaryName}`);
} catch {
  process.stderr.write(
    `relate: platform package "${pkg}" is not installed.\n` +
    `Try reinstalling: npm i -g @relateby/cli\n`
  );
  process.exit(1);
}

const result = spawnSync(binaryPath, process.argv.slice(2), { stdio: "inherit" });
process.exit(result.status ?? 1);
