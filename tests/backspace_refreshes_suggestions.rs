//! Regression test for the Backspace/bind-tty-special-chars bug: readline
//! continuously re-binds whatever key `stty erase` points to (DEL, almost
//! always) back to its own default, silently ignoring our `bind -x` for it,
//! unless `bind-tty-special-chars` is turned off. See ARCHITECTURE.md.
//!
//! Also covers a regression in `tty::preexec_clear_sequence` (the fix for
//! `enter_preexec_clears_stale_suggestions.rs`): the `DEBUG` trap fires not
//! just before the real submitted command but before every one of our own
//! `bind -x` handlers too (see `preexec_and_debug_trap`'s doc comment). An
//! earlier version of the fix applied the cursor-up correction
//! unconditionally, which is only valid for the real-command case; applied
//! here, before Backspace's own handler runs, it walked the cursor up into
//! the prompt row and reserved/scrolled from there — visibly shoving the
//! whole prompt line upward while leaving the actual block, one row down,
//! uncleared.

mod common;

use common::BashSession;
use std::time::Duration;

#[test]
fn backspace_triggers_our_handler_and_refreshes_suggestions() {
    let bin = env!("CARGO_BIN_EXE_rsreadline");
    let session = BashSession::spawn("git init\ncargo init\nsudo /etc/init.d/bluetooth restart\n");

    session.send_and_drain(format!("eval \"$({bin} init bash)\"\n").as_bytes());
    // Run a real command first, matching how a user's session actually
    // looks by the time they're typing something (not the very first byte
    // after sourcing .bashrc).
    session.send_and_drain(b"echo hi\n");

    session.send_and_drain(b"i");
    session.send_and_drain(b"n");
    session.send_and_drain(b"i"); // line is now "ini", suggestions showing

    let after_backspace = session.send_and_drain(b"\x7f"); // real DEL byte
    std::thread::sleep(Duration::from_millis(50));

    let text = String::from_utf8_lossy(&after_backspace);

    // Bug reproduction: bash's own default rubout is exactly "\x08\x1b[K"
    // and nothing else — our render never ran. If that's ALL we got back,
    // the fix regressed.
    assert_ne!(
        after_backspace.as_slice(),
        b"\x08\x1b[K",
        "backspace produced only bash's default erase output — our bind -x handler never ran:\n{text}"
    );

    // Positive check: our renderer drew a fresh suggestion for the
    // shortened query "in". Typing never selects anything (only Up/Down
    // does), so this must NOT be reverse-video highlighted.
    assert!(
        text.contains("init"),
        "expected a suggestion for the shortened query 'in' in the output:\n{text}"
    );
    assert!(
        !text.contains("\x1b[7m"),
        "typing must never select/highlight a suggestion, only Up/Down does:\n{text}"
    );

    // The DEBUG trap's harmless firing right before __rsreadline_backspace
    // itself runs must use the plain clear (no leading cursor-up) — with
    // config's default max_suggestions of 5, "\x1b[1A" can only be the
    // cursor-up correction, never return_to_start(5)'s "\x1b[5A".
    assert!(
        !after_backspace.windows(4).any(|w| w == b"\x1b[1A"),
        "backspace's DEBUG-trap clear used the cursor-up-adjusted sequence — \
         that's only correct right after Enter, not before our own handler runs:\n{text}"
    );
}
