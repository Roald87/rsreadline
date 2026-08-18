//! Regression test for the Backspace/bind-tty-special-chars bug: readline
//! continuously re-binds whatever key `stty erase` points to (DEL, almost
//! always) back to its own default, silently ignoring our `bind -x` for it,
//! unless `bind-tty-special-chars` is turned off. See ARCHITECTURE.md.

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

    // Positive check: our renderer draws the reverse-video-highlighted top
    // match, and "in" should still match at least one of the seeded
    // history entries.
    assert!(
        text.contains("\x1b[7m"),
        "expected our reverse-video suggestion marker in the output:\n{text}"
    );
    assert!(
        text.contains("init"),
        "expected a suggestion for the shortened query 'in' in the output:\n{text}"
    );
}
