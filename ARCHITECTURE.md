# Architecture

## Why `bind -x`, not a PTY-wrapper shell

GNU Readline has no "redraw after every keystroke" hook. Two options:
own the terminal (spawn bash in a PTY, do our own raw-mode line editing —
full control, but rsreadline becomes responsible for signal forwarding,
resize handling, prompt-boundary detection), or rebind individual keys via
`bind -x` and let bash keep doing all normal prompting/history/job control.
Chose the latter: less capable (see Enter, below) but far less surface area.

## Module layout

- `config.rs` — hand-rolled `key = value` config parser
- `history.rs` — `.bash_history` parsing (skips `HISTTIMEFORMAT` timestamp
  lines, trims trailing whitespace so near-duplicate entries dedupe cleanly)
- `matcher.rs` — substring matching, prefix-ranked, deduped, most-recent-first
- `tty.rs` — terminal size (`ioctl`/`TIOCGWINSZ`) and escape-sequence builders
- `bashgen.rs` — generates the `.bashrc` glue script
- `main.rs` — subcommand dispatch (`init bash`, `render`, `complete`)

## Zero dependencies

`dirs` → `$HOME` env var, `serde`/`toml` → hand-rolled parser for a 3-key
flat schema, `clap` → manual `match` over `env::args()`, `crossterm` → a
small `unsafe extern "C"` `ioctl` call for terminal size plus plain
string-building for ANSI sequences. `tty.rs`'s public functions are shaped
the way `crossterm` itself would be, so swapping to it later would only
touch that module's internals.

## Matching

Substring, prefix-ranked: entries starting with the typed text outrank ones
that merely contain it, most-recent-first within each group. Not fuzzy —
PSReadline itself only does prefix/subsequence, for the same reason a
mid-string match outranking an obvious prefix reads as wrong.

## Selecting a suggestion

Nothing is selected while you type — the block shows matches unhighlighted.
Down/Up select (first match / last match respectively, from "nothing"), and
— like bash's own history browsing — fill READLINE_LINE with the selected
suggestion's full text as a preview, not just highlight it. Enter is never
rebound (see below), so it needs no special handling here either: it just
submits whatever's currently in the line, which is already the selection.

Since Down/Up now overwrite READLINE_LINE, matching can no longer read the
query from READLINE_LINE the way typing does — cycling would end up
matching against the preview text instead of what you actually typed. A
separate `_RSREADLINE_QUERY` holds the real typed text; typing updates it,
cycling reads it.

Tab is bash's own native completion (readline's default `complete`) when
nothing is selected — not our suggestion box; this tool doesn't touch Tab
at all in that state, same as before this feature ever existed. Once Up/Down
has selected something, Tab becomes a no-op (Enter is how a selection gets
confirmed instead). That's the same "can't chain to the default action"
problem as Enter — `bind -x` can't intercept a key and still trigger the
native readline function — so Tab uses the same dynamic-rebind trick as
Up/Down: `__rsreadline_update` points `\C-i` at native `complete` when
`_RSREADLINE_SEL` is empty, or at our `__rsreadline_tab_noop` otherwise. The
no-op still calls `__rsreadline_update stay` (a `next_selected` direction
that keeps the current selection exactly as-is) rather than truly doing
nothing at the byte level — needed to repaint over the DEBUG-trap preexec
hook's harmless spurious clear (see below), which would otherwise make the
block visibly vanish until the next real keystroke. Regression test:
`tests/happy_path_selection_and_completion.rs`.

## Up/Down: rebinding, not reimplementing history

`bind -x` fully replaces a key's action — no "fall through to bash's
default." But the binding itself can be changed anytime, and `bind` can
point a key at a named function (`previous-history`) or a `bind -x` command.
So Up/Down are repointed dynamically: after every keystroke, `render`
reports the match count; non-zero → our cycle handler, zero → bash's real
history browsing. More correct than a file-based re-walk (uses bash's
actual in-memory history, including unflushed commands).

## `bind-tty-special-chars`

Readline continuously re-binds whatever key `stty erase` points to (DEL,
almost always) back to its own default, overriding any `bind`/`bind -x` for
it — confirmed with a real pty test, not just docs. The fix,
`bind 'set bind-tty-special-chars off'`, disables that re-binding entirely.
(Toggling `stty erase` itself around each command also works, but was
rejected: it's a terminal-wide setting, and a failed restore could break
Backspace in *every* program in that terminal, not just here.) Regression
test: `tests/backspace_refreshes_suggestions.rs`.

## Enter and the `DEBUG`-trap preexec hook

Enter is never rebound — `bind -x` can't intercept a key and still trigger
bash's own `accept-line`. Without it, the suggestion block sits on screen
untouched for as long as the submitted command runs (`PROMPT_COMMAND` only
cleans up before the *next* prompt), so short command output can end up with
leftover suggestion characters glued onto it.

Fix: bash's `DEBUG` trap runs a command immediately before every simple
command bash executes, including a just-submitted line (the same primitive
`bash-preexec` uses). It also fires before commands *inside* our own
`bind -x` handlers, so a `_RSREADLINE_BUSY` guard — set around each handler's
body — makes the preexec hook only actually clear when bash is running a
real command, not our own bookkeeping. One accepted gap: `DEBUG` fires
before each handler's very first statement too, before the flag is set; a
harmless spurious clear, since that handler's own final action always
redraws correctly right after. Regression test:
`tests/enter_preexec_clears_stale_suggestions.rs`.

## `read` silently drops empty fields

`__rsreadline_update` parses `render`'s tab-separated
`selected\tcount\tfill` output with `IFS=$'\t' read -r sel count fill`.
Once `selected`/`fill` could legitimately be empty (see above), this broke:
bash's `read` treats IFS whitespace characters (space/tab/newline) as
collapsible even with a custom IFS, silently dropping empty fields anywhere
but the last —
`IFS=$'\t' read -r a b c <<< $'\t3\t'` gives `a=3 b= c=`, not `a= b=3 c=`.
Every field after the first empty one shifts left, so `count` (used to
decide the Up/Down rebind) silently went missing. Switched the separator to
`\x01` (SOH): not whitespace, so `read` splits on it literally, and it can't
plausibly appear in real command text the way a stray tab or pipe might.

## Rendering

A fixed `max_suggestions`-line block is always drawn below the cursor
(unused slots cleared blank), so a shrinking match list never leaves stale
text. Escape sequences go to `/dev/tty` directly, not stdout, which carries
return values back to the bash glue. Below `min_terminal_height`, a
single warning line replaces the block instead of going silent.

Downward movement uses Cursor Down (`ESC[nB`), not a bare `\n` — a linefeed
at the bottom margin scrolls the screen, which invalidates
`SAVE_CURSOR`/`RESTORE_CURSOR`'s saved absolute position (neither tracks
scroll offset). `ESC[nB` clamps at the bottom row instead of scrolling.
