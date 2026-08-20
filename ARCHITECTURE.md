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

## `history -a` in `PROMPT_COMMAND`

Suggestions are matched against `history_file` on disk (`history::
load_entries`), but bash only appends to `HISTFILE` on shell exit by
default. Without a flush, a command run earlier in the same session
wouldn't suggest itself when retyped — confirmed with a real pty test, not
just docs. `__rsreadline_prompt_reset` runs `history -a` first, which
flushes bash's in-memory history to `HISTFILE` right after each command,
before the next prompt. Regression test:
`tests/prompt_reset_flushes_history_to_disk.rs`.

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

Downward movement inside the block uses Cursor Down (`ESC[nB`), not a bare
`\n` — a linefeed at the bottom margin scrolls the screen, which would
invalidate an absolute saved cursor position (nothing tracks scroll
offset), while `ESC[nB` clamps at the bottom row instead of scrolling.

That clamp has a cost, though: with the cursor already on the terminal's
last row, `ESC[nB` can't move down at all, so every line of the block lands
on that one clamped row and clobbers the line drawn before it — the block
renders as if it were empty (or, worse, wipes the prompt line's own
content, immediately papered over by bash's next redisplay). Confirmed by
actually pinning a real pty session's prompt to its last row, not just
reasoned about — see below.

The fix (`reserve_rows` / `return_to_start` in `tty.rs`) runs *before* a
clamped loop: it moves down `block_size` rows using Index (`ESC D`) instead
of `ESC[nB` — Index scrolls at the bottom margin exactly like a bare `\n`
would, guaranteeing the room exists — then a relative `ESC[nA` jump back up
by the same count. Because both the forced scroll (if any) and the
cursor's own position move together, undoing our own relative movement
lands back at the exact starting row *and column* regardless of whether a
scroll happened — unlike `SAVE_CURSOR`/`RESTORE_CURSOR`. Index is used
rather than a literal `\n` byte for the same reason `ESC[nB` was originally
chosen over one: a bare `\n` can be silently turned into CR+LF by the
terminal's `ONLCR` setting, resetting the column; `ESC D` is a distinct
escape sequence, not the byte `0x0A`, so `ONLCR` never touches it.

That reservation is only safe at *boundary* moments, though — between one
line-edit session and the next — which is why only `clear_sequence` (called
from `PROMPT_COMMAND` and the DEBUG-trap preexec) uses it, while
`render_sequence`/`warning_sequence` (called on every keystroke, from
`__rsreadline_update`, still mid-edit) go back to plain `SAVE_CURSOR`/
`RESTORE_CURSOR` and never force a scroll. Confirmed the hard way: an
earlier version of this fix made `render_sequence` reserve rows too, and
typing near the bottom row made the prompt line grow stray leading
whitespace, worse with more keystrokes. Cause: bash's readline keeps its
*own* internal model of what's already on screen so it can do incremental,
diff-based redraws — it has no idea our raw writes to `/dev/tty` just
scrolled the terminal. Restoring the cursor to the exact right spot doesn't
fix that: the scroll shifts real on-screen content out from under
readline's stale model without telling it, so its next incremental redraw
computes against the wrong assumptions. `clear_sequence`'s callers don't
have this problem — both fire at a point where readline isn't mid-redisplay
(preexec after `accept-line` has already returned control to bash;
`PROMPT_COMMAND` before the next `readline()` call even starts), so nothing
has a stale model for the scroll to invalidate. By the time the user starts
typing, room has already been reserved by the preceding `clear_sequence`
call, so `render_sequence`'s clamp is not expected to actually bind.

## Deleting a suggestion (Shift+Delete)

Shift+Delete removes the currently selected suggestion from the history
file entirely — every occurrence of that exact command text, not just the
most recent one, so a polluting entry (a typo, a stray `cd`/`ls`, etc.)
actually stops being suggested for good. `HISTIGNORE` can't do this itself:
it only governs what bash writes *going forward*, not lines already sitting
in the file.

This reuses `render` (a `direction == "delete"` value) rather than adding a
new subcommand: `render` already takes "current state + a direction" and
returns "new state," and a delete is just that same cycle with a disk
mutation folded in — mutate, reload, recompute matches, then fall into the
same select/draw/return tail every other direction uses. A separate
subcommand would still need a second `render` call right after for the
redraw, which breaks the "every handler ends in exactly one
`__rsreadline_update` call" pattern every other `bashgen.rs` handler
follows.

`next_selected`'s `"delete"` arm clamps rather than reproducing `"stay"`'s
exact-no-op behavior: `current.map(|i| i.min(count - 1))`. This is the one
direction where the match list can legitimately have a different length
than when `current` was chosen — count doesn't necessarily shrink by
exactly one, since `matcher::suggest` truncates to `max_suggestions`, so
deleting the selected entry can let a previously truncated match backfill
the same slot instead. The formula handles both cases: same slot stays
selected if still in bounds (backfill, or the deleted entry wasn't last),
clamped to the new last index otherwise.

On-disk removal pairs each matching line with its preceding
`HISTTIMEFORMAT` timestamp-comment line (if any), checked per-occurrence
against that occurrence's own immediate predecessor — not "the nearest
timestamp line anywhere nearby" — so a second occurrence sitting near an
unrelated timestamp comment doesn't accidentally take it out too. Written
atomically (temp file + rename): this is the one place rsreadline mutates
the user's live shell history rather than just reading it, and a plain
truncate-then-write that's interrupted partway through would turn "delete
one line" into "wipe the whole file." The original file's permission bits
are copied onto the temp file before the rename, since `.bash_history` can
hold secrets typed into commands and a privacy-conscious `chmod 600`
shouldn't get silently loosened to umask-default permissions. Rejoining
with `\n` after `str::lines()` normalizes any CRLF line endings to LF, even
on untouched lines — accepted, since this is a Linux-only tool and
`.bash_history` is effectively always LF in practice.

In `bashgen.rs`, `__rsreadline_delete_selected` resets `READLINE_LINE` to
`_RSREADLINE_QUERY` *before* calling `__rsreadline_update delete`:
`READLINE_LINE` is still holding the about-to-be-deleted entry's preview
text, and `__rsreadline_update` only overwrites it when the response's
`fill` comes back non-empty. If the delete leaves nothing selected, `fill`
is empty, so this reset is what's actually left on the line afterward — if
something does end up selected post-delete, `fill` is non-empty and
overwrites it again anyway. The binding is static, not dynamically rebound
like Up/Down/Tab (bash has no competing native binding for it to restore),
so the handler is reachable with nothing selected too; that branch still
calls `__rsreadline_update stay` rather than doing nothing, same as
`__rsreadline_tab_noop`, to repaint over the DEBUG-trap preexec hook's
harmless spurious clear.

Two things here are only verified against this repo's own pty test, not
against reality at large, in the same spirit as the `ESC[nB` clamp bug and
`bind-tty-special-chars` above:
- The `\e[3;2~` byte sequence is the standard xterm/most-terminal-emulators
  CSI code for Shift+Delete, but terminal emulators vary — this is only
  confirmed against whatever bytes this repo's own pty test sends, not
  against every real terminal a user might run this in.
- `history -a` (in `__rsreadline_prompt_reset`) tracks what it's already
  flushed via bash's own in-memory bookkeeping, not by diffing file
  content — rewriting `.bash_history` out from under bash mid-session
  could plausibly confuse a later flush. Tested directly: after a delete,
  a freshly run command still gets flushed and suggests itself correctly,
  and the deleted entries don't resurrect.

Regression test: `tests/shift_delete_removes_selected_suggestion.rs`.
