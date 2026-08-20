# Architecture

This file covers decisions that span the codebase — not the internal
mechanics of any one module. Those live as doc comments at the point of
definition (start with each module's own `//!` doc comment); grep for
"Regression test" in `src/` to find the pty test backing a given claim.

## Why `bind -x`, not a PTY-wrapper shell

GNU Readline has no "redraw after every keystroke" hook. Two options: own
the terminal (spawn bash in a PTY, do our own raw-mode line editing — full
control, but rsreadline becomes responsible for signal forwarding, resize
handling, prompt-boundary detection), or rebind individual keys via `bind
-x` and let bash keep doing all normal prompting/history/job control. Chose
the latter: less capable — `bind -x` replaces a key's action outright, it
can't intercept a key and still fall through to that key's native readline
function, which is why Enter is never rebound and Up/Down/Tab have to be
dynamically re-pointed instead of reimplemented (see `bashgen.rs`) — but far
less surface area.

## Zero dependencies

`dirs` → `$HOME` env var, `serde`/`toml` → hand-rolled parser for a 3-key
flat schema, `clap` → manual `match` over `env::args()`, `crossterm` → a
small `unsafe extern "C"` `ioctl` call for terminal size plus plain
string-building for ANSI sequences. `tty.rs`'s public functions are shaped
the way `crossterm` itself would be, so swapping to it later would only
touch that module's internals.

## The binary ⇄ generated-script contract

`rsreadline init bash` (`bashgen.rs`) emits a self-contained bash script
that `.bashrc` evals once. From then on, every keystroke is one round trip:
the script calls `rsreadline render <line> <point> <selected> <direction>`
(`main.rs::cmd_render`), which recomputes matches, draws (or clears) the
suggestion block directly to `/dev/tty`, and hands back the new selection
state for the script to store — `render` is otherwise stateless, so
`direction`'s meaning (typing vs. cycling vs. deleting) and what counts as
"the query" are entirely the caller's responsibility, documented where the
script builds that call in `bashgen.rs::update_function`.

The one invariant that spans both halves and is easy to break when adding a
new key handler: every function bash calls directly via `bind -x` must set
`_RSREADLINE_BUSY=1` before doing anything and clear it before returning.
Enter can't be rebound, so a `DEBUG` trap is what clears the suggestion
block once a submitted command starts running — but that trap also fires
before commands *inside* our own handlers, and the busy flag is what stops
it from clearing the block out from under a handler that's still using it.
See `bashgen.rs::preexec_and_debug_trap` and the handler functions below it
for the mechanics.

## Rendering must not scroll mid-edit

Suggestions are redrawn on every keystroke with raw escape sequences to
`/dev/tty`, while bash's own readline is simultaneously doing incremental,
diff-based redraws of the prompt line from its own on-screen model — a
model that knows nothing about our writes. If one of ours triggers an
actual terminal scroll, that model goes stale and readline's next redraw
computes against the wrong assumptions (observed as the prompt line growing
stray padding, worse the more scroll had occurred). So every sequence
written while an edit is in progress is restricted to movements that
provably can't scroll; the one place a scroll is safe — because it only
runs at a boundary between edits, never mid-edit — is documented in
`tty.rs` (`reserve_rows`).
