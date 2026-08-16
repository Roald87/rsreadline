# rsreadline

Live, PSReadline-style command suggestions for bash, built against your
`.bash_history`. Single zero-dependency Rust binary, wired into bash via
`bind -x` — bash keeps doing all normal prompting, history, and job control.

While you type, up to 5 matching history entries are shown on the lines
below the prompt, with the selected one shown in reverse video (no fixed
color, so it can't clash with your terminal theme). Tab completes the
selected suggestion; Up/Down cycle through suggestions when any are shown,
otherwise they fall back to bash's own history browsing.

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
3. Press Down/Up repeatedly → the reverse-video highlight moves between
   suggestions and wraps
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

- The selected suggestion is highlighted with reverse video, not a fixed
  color, so it can't clash with a terminal theme
- Matching is substring, ranked prefix-first, most-recent-first within each
  rank — not fuzzy
- Enter is intentionally never rebound (bind -x can't intercept a key and
  still trigger bash's own accept-line). A `DEBUG` trap-based preexec hook
  clears the suggestion block right before your submitted command starts
  running, closing the window where its own output could collide with
  leftover suggestion text — see the "Why this trap exists" comment in the
  `init bash` output for the full mechanism. Not a hard guarantee for every
  possible bash internal quirk, but the corruption seen in early live testing
  (e.g. a real error message ending up with stray suggestion-text characters
  glued onto it) should no longer occur in normal use
- Near the bottom of the terminal, drawing the block can still trigger a
  scroll that breaks the cursor save/restore trick, even above
  `min_terminal_height`
- `PROMPT_COMMAND` is treated as a plain string; a `PROMPT_COMMAND` array
  (bash 5.1+) is not specifically handled
