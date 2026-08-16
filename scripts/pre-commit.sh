#!/usr/bin/env bash
set -euo pipefail

echo "pre-commit: cargo fmt --check"
if ! cargo fmt --all -- --check; then
    echo "pre-commit: formatting issues found — run 'cargo fmt' and re-stage" >&2
    exit 1
fi

echo "pre-commit: cargo clippy"
if ! cargo clippy --all-targets --all-features -- -D warnings; then
    echo "pre-commit: clippy found issues — fix them (or try 'cargo clippy --fix') and re-stage" >&2
    exit 1
fi
