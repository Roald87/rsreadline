#!/usr/bin/env bash
set -euo pipefail

if [ $# -ne 1 ]; then
    echo "usage: scripts/release.sh <version>   (semver, no 'v' prefix, e.g. 0.2.0)" >&2
    exit 1
fi

version="$1"
if ! [[ "$version" =~ ^[0-9]+\.[0-9]+\.[0-9]+$ ]]; then
    echo "error: version must be semver X.Y.Z (got '$version')" >&2
    exit 1
fi

cd "$(git rev-parse --show-toplevel)"

if [ -n "$(git status --porcelain)" ]; then
    echo "error: working tree is not clean — commit or stash changes first" >&2
    exit 1
fi

if git rev-parse "v$version" >/dev/null 2>&1; then
    echo "error: tag v$version already exists" >&2
    exit 1
fi

# Only the [package] version, not a same-named key elsewhere (e.g. a
# dependency's own "version ="): the range stops at the first match.
sed -i "0,/^version = /s/^version = \".*\"/version = \"$version\"/" Cargo.toml

# `cargo check` resyncs Cargo.lock's own self-entry to match Cargo.toml —
# no `cargo update` needed, and it leaves dependency versions untouched.
cargo check --quiet

git add Cargo.toml Cargo.lock
git commit -m "Bump version to $version"
git tag "v$version"
git push origin main
git push origin "v$version"

echo "Released v$version — the release workflow will build and publish the binary."
