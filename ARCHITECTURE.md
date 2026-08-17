# Architecture

## Why `bind -x`, not a PTY-wrapper shell

GNU Readline (bash's line editor) has no "redraw after every keystroke" hook.
Two ways to get live suggestions were considered:

- **A PTY-wrapper shell**: rsreadline owns the terminal, spawns bash inside a
  pseudo-terminal, does its own raw-mode line editing, and forwards finished
  lines to bash. Full control (including over Enter), but rsreadline becomes
  responsible for signal forwarding, resize handling, prompt-boundary
  detection — much more surface area.
- **`bind -x` keystroke rebinding** (chosen): rsreadline stays a plain helper
  binary invoked from `.bashrc`. Individual keys are rebound via `bind -x` so
  bash keeps doing all normal prompting, history, and job control; rsreadline
  is only invoked synchronously on specific keystrokes.

The second option trades some correctness/robustness (see the Enter section
below) for staying out of bash's way entirely.

## Module layout

- `config.rs` — hand-rolled `key = value` config parser
- `history.rs` — `.bash_history` parsing (skips `HISTTIMEFORMAT` timestamp
  lines, trims trailing whitespace so near-duplicate entries dedupe cleanly)
- `matcher.rs` — substring matching, prefix-ranked, deduped, most-recent-first
- `tty.rs` — terminal size (`ioctl`/`TIOCGWINSZ`) and escape-sequence builders
- `bashgen.rs` — generates the `.bashrc` glue script
- `main.rs` — subcommand dispatch (`init bash`, `render`, `complete`)

## Zero dependencies

The shipped binary has no crates: `dirs` → `$HOME` env var, `serde`/`toml` →
hand-rolled parser for a 3-key flat schema, `clap` → manual `match` over
`env::args()`, `crossterm` → a small `unsafe extern "C"` `ioctl` call for
terminal size plus plain string-building for ANSI sequences. `tty.rs`'s
public functions are shaped the way `crossterm` itself would be, so swapping
to it later would only touch that module's internals.

## Matching: substring, prefix-ranked

Entries containing the typed text anywhere match; ones that *start with* it
rank above ones that merely *contain* it, most-recent-first within each
group. Plain prefix-only matching was the original design but was widened
after the first round of feedback — not fuzzy, since PSReadline itself only
does prefix/subsequence for the same reason a stray mid-string match
outranking an obvious prefix match reads as wrong.

## Up/Down: rebinding, not reimplementing history

`bind -x` fully replaces a key's action for a given press — there's no
"fall through to bash's default" mid-keypress. But the *binding itself* can
be changed at any time by other code in the same shell, and `bind` can point
a key at either a named readline function (`previous-history`/
`next-history`) or a `bind -x` shell command. So instead of reimplementing
history browsing, Up/Down are **dynamically repointed**: after every
keystroke, `render` reports back the current match count; if it's non-zero,
Up/Down are bound to our cycle handler, otherwise they're pointed back at
bash's real, native history browsing. This is more correct than a
file-based re-walk would have been (it uses bash's actual in-memory
history, including commands not yet flushed to disk).

## Enter and the `DEBUG`-trap preexec hook

Enter is intentionally never rebound: there's no way to both intercept a key
via `bind -x` *and* still trigger bash's real `accept-line` from inside the
handler — the binding fully replaces the key's default action with nothing
to chain back to.

Without touching Enter, the suggestion block drawn below the cursor sits on
screen, untouched, for the entire time your submitted command runs —
`PROMPT_COMMAND` only cleans it up before the *next* prompt, i.e. after the
command has already finished. Early live testing hit this directly: a real
`Could not locate Gemfile` error came out with leftover suggestion
characters (`ve --`) glued onto it, because the command's own output was
narrower than the stale suggestion text it was overwriting.

The fix uses bash's `DEBUG` trap: `trap CMD DEBUG` runs `CMD` immediately
before every simple command bash is about to execute, including a
just-submitted line — the same primitive the community `bash-preexec`
project builds "preexec" hooks out of for other tools. Verified empirically
(not just from docs) that it also fires before every command *inside* our
own `bind -x` handler functions, and — usefully — does **not** re-fire for
commands run by the trap handler itself (no runaway recursion).

That first fact means clearing unconditionally on every `DEBUG` firing would
also clear the block repeatedly while the user is still typing, before our
own handler gets a chance to redraw it. The fix is a `_RSREADLINE_BUSY`
guard: every `bind -x` entry point (insert/backspace/tab/up/down) sets it
around its own body, and the preexec handler skips clearing whenever it's
set — so it only actually fires for commands bash runs on its own behalf.

One accepted imprecision: `DEBUG` fires before each handler's very first
statement too, before that handler has had a chance to set the flag. This is
harmless — the handler's own final action is always a fresh redraw that
overwrites it within the same invocation, faster than a terminal could ever
visibly flicker — and is the trade-off for keeping the guard a single flag
instead of a much more elaborate (and fragile) "is this command text one of
ours" pattern-matcher.

## Rendering

A fixed `max_suggestions`-line block is always drawn below the cursor
(unused slots as cleared blank lines), so a shrinking match list never
leaves stale text behind. Escape sequences go to `/dev/tty` directly, not
stdout, since stdout carries return values back to the bash glue in some of
these subcommands. Below `min_terminal_height`, a single warning line
replaces the block rather than going silent.

Downward movement uses Cursor Down (`ESC[nB`), not a bare `\n`. A linefeed
at the terminal's bottom margin scrolls the whole screen — which silently
invalidates `SAVE_CURSOR`/`RESTORE_CURSOR`'s saved *absolute* position, since
neither tracks the scroll offset. This caused real corruption in live
testing (a warning message showing up with only its tail surviving,
overwritten by bash's next redraw). `ESC[nB` clamps at the bottom row
instead of scrolling, so it doesn't trigger the problem in the first place.
