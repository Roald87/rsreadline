# Contributing

## Setup

Install the pre-commit hook once per clone (hooks aren't tracked by git):

```sh
ln -sf ../../scripts/pre-commit.sh .git/hooks/pre-commit
chmod +x .git/hooks/pre-commit
```

It runs `cargo fmt --check` and `cargo clippy -D warnings` before every commit.

## Checks

```sh
cargo test
cargo clippy --all-targets
cargo fmt --all -- --check
```

## Manual verification (not automatable)

Needs a real terminal — type a known history prefix and check suggestions
appear, cycle Up/Down, Tab-complete, clear the line and confirm Up/Down falls
back to normal history browsing, submit a command, resize below 15 rows, and
type `'`, `"`, `\`, `` ` ``, `$` to check they still insert correctly.
