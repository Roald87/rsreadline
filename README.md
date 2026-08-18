# rsreadline

Live, PSReadline-style command suggestions for bash, built against your
`.bash_history`. Zero-dependency Rust binary, wired into bash via `bind -x`.

While you type, up to 5 matching history entries show below the prompt.
Select a suggestion using the arrow keys and press enter to submit.

## Build

```sh
cargo build --release
```

## Setup

Add to `.bashrc`:

```sh
eval "$(/path/to/rsreadline init bash)"
```

Requires a terminal at least 15 lines tall (configurable).

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

`cargo test` includes a couple of `tests/*.rs` integration tests that spawn a
real bash session in a pty and send literal keystrokes — some bash/readline
behavior only shows up at runtime, not in the generated script's text (see
ARCHITECTURE.md).
