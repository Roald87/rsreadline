# rsreadline

Live, [PSReadline](https://github.com/PowerShell/PSReadLine)-style command suggestions
for bash, built against your `.bash_history`. Zero-dependency Rust binary, wired into
bash via `bind -x`.

While you type, up to 5 matching history entries show below the prompt.
Select a suggestion using the arrow keys and press enter to submit. Press
Shift+Delete on a selected suggestion to remove it from your history
entirely (all occurrences).

## Install

```sh
curl -fsSL https://raw.githubusercontent.com/Roald87/rsreadline/main/install.sh | bash
```

Downloads a prebuilt binary to `~/.local/bin/rsreadline` (Linux x86_64 only for
now). Or build from source:

```sh
cargo build --release
```

## Setup

Add to `.bashrc`:

```sh
eval "$(rsreadline init bash)"
```

(or `/path/to/rsreadline` if it's not on your `PATH`).

Requires a terminal at least 15 lines tall (configurable).

## Uninstall

Remove the `eval "$(rsreadline init bash)"` line from `.bashrc`, then:

```sh
rm ~/.local/bin/rsreadline
rm -rf ~/.config/rsreadline   # if you had a config.toml
```

## Config

`~/.config/rsreadline/config.toml`, all fields optional:

```toml
history_file = "~/.bash_history"
max_suggestions = 5
min_terminal_height = 15
```

Changing `max_suggestions` requires re-sourcing `.bashrc` since it's baked
into the generated bash glue.

## Test / lint

```sh
cargo test
cargo clippy --all-targets
cargo fmt --all -- --check
```

Pre-commit hook (checks the above, not tracked by git so install once per
clone):

```sh
ln -sf ../../scripts/pre-commit.sh .git/hooks/pre-commit
chmod +x .git/hooks/pre-commit
```

`cargo test` includes several `tests/*.rs` integration tests that spawn a
real bash session in a pty and send literal keystrokes — some bash/readline
behavior only shows up at runtime, not in the generated script's text (see
ARCHITECTURE.md).

## Release

```sh
scripts/release.sh 0.2.0
```

Bumps the version in `Cargo.toml`/`Cargo.lock`, commits, tags, and pushes —
pushing the tag triggers the release workflow that builds and publishes the
binary.
