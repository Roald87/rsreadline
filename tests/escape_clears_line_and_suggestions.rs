//! End-to-end test for Esc clearing the typed line and dismissing the
//! suggestion block, PSReadLine-style — and that typing afterward starts a
//! fresh query, proving state was actually reset, not just the visible
//! line.
//!
//! Waits past `keyseq-timeout` (50ms, see `bashgen::header`) before
//! checking, since bare `\e` is ambiguous with longer escape sequences
//! until then.
//!
//! Drives a real bash session in a pty (see tests/common) since none of
//! this is observable from generated-script text alone.

#![allow(clippy::expect_used)]

mod common;

use common::BashSession;
use std::time::Duration;

#[test]
fn escape_clears_line_and_dismisses_suggestions() {
    let bin = env!("CARGO_BIN_EXE_rsreadline");
    let history = "git status\ngit commit\n";
    let session = BashSession::spawn(history);
    session.send_and_drain(format!("eval \"$({bin} init bash)\"\n").as_bytes());
    session.send_and_drain(b"true\n"); // realistic warm-up

    // --- Phase 1: type "git", select a match ---
    let mut typed = Vec::new();
    for byte in b"git" {
        typed = session.send_and_drain(&[*byte]);
    }
    let text = String::from_utf8_lossy(&typed);
    assert!(
        text.contains("git status") && text.contains("git commit"),
        "expected both suggestions after typing 'git':\n{text}"
    );

    let after_down = session.send_and_drain(b"\x1b[B");
    let text = String::from_utf8_lossy(&after_down);
    assert!(
        text.contains("\x1b[7m"),
        "expected a highlighted selection after Down:\n{text}"
    );

    // --- Phase 2: bare Esc discards the line and the suggestion block ---
    session.send(b"\x1b");
    std::thread::sleep(Duration::from_millis(150));
    let after_escape = session.drain(Duration::from_millis(300));
    let text = String::from_utf8_lossy(&after_escape);
    assert!(
        !text.contains("git status") && !text.contains("git commit"),
        "suggestions must be gone after Esc:\n{text}"
    );
    assert!(
        !text.contains("\x1b[7m"),
        "no selection highlight should remain after Esc:\n{text}"
    );

    // --- Phase 3: state is fully reset, not just the visible line —
    // typing again starts a fresh query and suggestions reappear normally.
    let mut typed = Vec::new();
    for byte in b"git" {
        typed = session.send_and_drain(&[*byte]);
    }
    let text = String::from_utf8_lossy(&typed);
    assert!(
        text.contains("git status") && text.contains("git commit"),
        "expected suggestions to work normally after Esc reset state:\n{text}"
    );
}
