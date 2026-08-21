# CLAUDE.md

This file provides guidance to Claude Code (claude.ai/code) when working with code in this repository.

## What this is

`rsreadline` is a zero-dependency Rust binary that adds live, PSReadLine-style
command suggestions to bash, driven by `.bash_history`. It has no runtime
dependencies — see ARCHITECTURE.md ("Zero dependencies") for what replaces
`dirs`/`serde`/`clap`/`crossterm` and why.

## Commands

```sh
cargo build --release        # release binary at target/release/rsreadline
cargo test --bin rsreadline  # unit tests only
cargo test --tests           # integration tests only (real pty sessions, slower)
cargo test                   # everything
cargo test <test_name>       # a single test, e.g. cargo test backspace_triggers_our_handler
cargo clippy --all-targets --all-features -- -D warnings
cargo fmt --all -- --check   # cargo fmt --all to fix
```

CI (`.github/workflows/ci.yml`) runs build, `fmt --check`, clippy (`-D
warnings`), unit tests, then integration tests, in that order.

The pre-commit hook (`scripts/pre-commit.sh`, runs fmt + clippy) isn't
tracked by git config — install it once per clone:

```sh
ln -sf ../../scripts/pre-commit.sh .git/hooks/pre-commit
chmod +x .git/hooks/pre-commit
```

Release: `scripts/release.sh <version>` bumps `Cargo.toml`/`Cargo.lock`,
commits, tags, and pushes; the pushed tag triggers `.github/workflows/release.yml`
to build and publish the Linux x86_64 binary.

## Architecture

Read `ARCHITECTURE.md` first — it covers the cross-cutting decisions (why
`bind -x` instead of owning the terminal, the binary⇄generated-script
contract, and the "rendering must not scroll mid-edit" constraint) that
span multiple files and are easy to violate by accident when adding a new
key handler. Module-level rationale lives as `//!` doc comments at the top
of each `src/*.rs` file instead of being duplicated here.

- `main.rs` — subcommand dispatch: `--version`, `init bash` (calls into
  `bashgen`), `render <line> <point> <selected> <direction>` (the per-keystroke
  round trip).
- `bashgen.rs` — generates the bash glue script `init bash` prints. This is
  the biggest and most subtle file: one `bind -x` handler per key, a `DEBUG`
  trap that clears the suggestion block before a submitted command runs, and
  a `PROMPT_COMMAND` hook. Read its module doc comment and
  `preexec_and_debug_trap`'s doc comment before touching trap/handler timing.
- `tty.rs` — terminal size (`ioctl`) and the raw ANSI sequences for
  drawing/clearing the suggestion block. Cursor-movement choices here
  (`CURSOR_DOWN_1` vs `INDEX_DOWN_1` vs absolute movement) encode which
  contexts are safe to scroll in — read the doc comments before adding a new
  sequence.
- `matcher.rs` — ranks history entries against the typed query.
- `history.rs` — parses `.bash_history` and removes entries (used by
  Shift+Delete).
- `config.rs` — hand-rolled `key = value` parser for
  `~/.config/rsreadline/config.toml`.

There is no `lib.rs` — this is a binary-only crate, so integration tests
can't import internal modules; they interact only via `env!("CARGO_BIN_EXE_rsreadline")`
and stdin/stdout/pty.

## Testing bash/readline behavior

Some bugs (stale escape-sequence state, cursor-position drift, key-binding
interference from `stty`) only show up against a real interactive bash
session, not from inspecting the generated script's text. `tests/*.rs`
integration tests spawn a real `bash --norc --noprofile` in a pty
(`tests/common`) and send literal keystrokes. When diagnosing this class of
bug, prefer reproducing it with a pty test over reasoning about the escape
sequences statically — and note that raw byte capture alone can't see
terminal-grid-only corruption (stale characters glued from an earlier,
wider write): what you *can* assert is escape-sequence structure/ordering
and exact byte shape, not loose `contains()` checks on the visible text —
see `tests/enter_preexec_clears_stale_suggestions.rs` and
`tests/happy_path_selection_and_completion.rs` for examples of both the
technique and this pitfall.

## Commenting

When writing comments or docs, always try to make it as short as possible. 
- There is no need to explain the whole design history in the comment. 
- Do not use filler words, hedging etc. 
- Avoid inflated phrases: One of the simplest ways to make your writing more concise is to avoid “inflated” phrases that use several words where just one or two would be sufficient. "A majority of" -> Most, At all times -> Always
- Avoid redundnacies: Advance planning -> Planning, Alternative choice ->  Alternative

and you probably have dozens of other tips in your memory.
