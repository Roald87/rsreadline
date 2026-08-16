# rsreadline

Live, PSReadline-style command suggestions for bash, built against your
`.bash_history`. Single zero-dependency Rust binary; no color, plain text,
wired into bash via `bind -x` — bash keeps doing all normal prompting,
history, and job control.

While you type, up to 5 matching history entries are shown on the lines
below the prompt. Tab completes the selected suggestion; Up/Down cycle
through suggestions when any are shown, otherwise they fall back to bash's
own history browsing.

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
get a single-line notice instead of the suggestion block, rather than going
silent.

## Config

`~/.config/rsreadline/config.toml`, all fields optional:

```toml
history_file = "~/.bash_history"
max_suggestions = 5
min_terminal_height = 15
```

Changing `max_suggestions` requires re-sourcing `.bashrc` (or re-running the
`eval` line) since it's baked into the generated bash glue.

## Development

Install the pre-commit hook once per clone (hooks aren't tracked by git):

```sh
ln -sf ../../scripts/pre-commit.sh .git/hooks/pre-commit
chmod +x .git/hooks/pre-commit
```

It runs `cargo fmt --check` and `cargo clippy -D warnings` before every commit.

```sh
cargo test
cargo clippy --all-targets
cargo fmt --all -- --check
```

## Manual verification (not automatable)

1. In a real terminal ≥15 rows: `eval "$(./target/release/rsreadline init bash)"`
2. Point `history_file` at a sample history with known entries; start typing a
   known prefix → up to 5 matches render below the current line
3. Press Down/Up repeatedly → the `> ` marker moves between suggestions and
   wraps
4. Press Tab → the line becomes the selected suggestion, cursor at end
5. Clear the line (Ctrl-U), press Up/Down → falls back to bash's own native
   history browsing
6. Press Enter with suggestions visible → command executes correctly (a
   brief leftover-suggestion-line glimpse right after Enter is a known,
   accepted v1 limitation)
7. Resize the terminal below 15 rows → the block is replaced by the
   single-line "suggestions hidden" notice
8. Type a line containing `'`, `"`, `\`, `` ` ``, `$` → each still inserts
   correctly

## Known v1 limitations

- No color; the selected suggestion is marked with a plain `> ` prefix
- Matching is substring, ranked prefix-first, most-recent-first within each
  rank — not fuzzy
- A brief stale-suggestion-line glimpse is possible right after pressing
  Enter (see [Key mechanics #6] — Enter is intentionally not intercepted)
- Near the bottom of the terminal, drawing the block can still trigger a
  scroll that breaks the cursor save/restore trick, even above
  `min_terminal_height`
- `PROMPT_COMMAND` is treated as a plain string; a `PROMPT_COMMAND` array
  (bash 5.1+) is not specifically handled
