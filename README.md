# rsreadline

Live, PSReadline-style command suggestions for bash, built against your
`.bash_history`. Zero-dependency Rust binary, wired into bash via `bind -x`.

While you type, up to 5 matching history entries show below the prompt, with
the selected one in reverse video. Tab completes it; Up/Down cycle through
suggestions, or fall back to bash's normal history browsing when none are
shown.

## Build

```sh
cargo build --release
```

## Setup

Add to `.bashrc`:

```sh
eval "$(/path/to/rsreadline init bash)"
```

Requires a terminal at least 15 lines tall (configurable); shorter terminals
get a single-line notice instead of the suggestion block.

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

### Manual verification (not automatable)

Needs a real terminal — type a known history prefix and check suggestions
appear, cycle Up/Down, Tab-complete, resize below 15 rows, and type `'`, `"`,
`\`, `` ` ``, `$` to check they still insert correctly.

## Known limitations

- Matching is substring, prefix-ranked, most-recent-first — not fuzzy
- Enter is never rebound, so there's a brief window between submitting a
  command and its output starting where stale suggestion text could still be
  visible; a preexec hook closes most of that window but not every edge case
- Long suggestion lines that wrap onto a second terminal row aren't accounted
  for in the block's height math
- `PROMPT_COMMAND` is assumed to be a plain string, not bash 5.1+'s array form

See `ARCHITECTURE.md` for why it's built this way.
