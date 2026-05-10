#!/usr/bin/env bash
# Local CI check — mirrors .github/workflows/ci.yml exactly.
# Run before pushing to catch failures before they hit the remote.
#
# Usage: ./scripts/ci-local.sh

set -euo pipefail

BOLD='\033[1m'
GREEN='\033[0;32m'
RED='\033[0;31m'
RESET='\033[0m'

pass() { echo -e "${GREEN}✓${RESET} $1"; }
fail() { echo -e "${RED}✗${RESET} $1"; }

step() {
    echo ""
    echo -e "${BOLD}── $1${RESET}"
}

run() {
    local label="$1"; shift
    if "$@"; then
        pass "$label"
    else
        fail "$label"
        exit 1
    fi
}

cd "$(dirname "$0")/.."

step "Formatting"
run "cargo fmt --check" cargo fmt --check

step "Clippy (deny warnings)"
run "cargo clippy --all-targets -- -D warnings" cargo clippy --all-targets -- -D warnings

step "Build"
run "cargo build" cargo build

step "Tests"
run "cargo test" cargo test

echo ""
echo -e "${GREEN}${BOLD}All checks passed.${RESET}"
