//! Regression test: Up/Down must cycle suggestions even when the terminal
//! is in DECCKM (application cursor-key) mode, where the arrow keys send
//! `\eOA`/`\eOB` instead of `\e[A`/`\e[B`.
//!
//! How a normal terminal ends up there: a full-screen-ish program (e.g.
//! `dotnet build`'s terminal logger) switches the terminal into
//! application cursor-key mode with `\e[?1h` and is expected to switch it
//! back with `\e[?1l` on exit. Kill it with Ctrl+C mid-run and the reset
//! never happens — the terminal stays in application mode, so Down now
//! emits `\eOB`. If we only bind `\e[B`, that keystroke falls through to
//! bash's native `next-history` and suggestion cycling looks broken until
//! the next program resets the mode.
//!
//! A raw pty has no terminal emulator tracking DECCKM, so this test can't
//! flip the real mode — it sends the `\eO`-form bytes directly, which is
//! exactly what a real terminal in that state delivers.

mod common;

use common::BashSession;

#[test]
fn down_in_application_cursor_mode_selects_a_suggestion() {
    let bin = env!("CARGO_BIN_EXE_rsreadline");
    let history = "echo alpha\necho beta\necho gamma\n";
    let session = BashSession::spawn(history);
    session.send_and_drain(format!("eval \"$({bin} init bash)\"\n").as_bytes());
    session.send_and_drain(b"true\n");

    for byte in b"echo" {
        session.send_and_drain(&[*byte]);
    }

    // Application-mode Down (\eOB), not the usual \e[B.
    let after_down = session.send_and_drain(b"\x1bOB");
    let text = String::from_utf8_lossy(&after_down);
    assert!(
        text.contains("\x1b[7mecho gamma\x1b[0m"),
        "expected 'echo gamma' highlighted after application-mode Down:\n{text}"
    );
    assert!(
        text.ends_with("echo gamma"),
        "expected the line filled with the selection after application-mode Down:\n{text}"
    );

    // Application-mode Up (\eOA) cycles: from the first row it wraps to the last.
    let after_up = session.send_and_drain(b"\x1bOA");
    let text = String::from_utf8_lossy(&after_up);
    assert!(
        text.contains("\x1b[7mecho alpha\x1b[0m"),
        "expected application-mode Up to wrap to 'echo alpha':\n{text}"
    );
}
