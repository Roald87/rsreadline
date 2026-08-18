//! End-to-end happy-path test for the selection model:
//!
//! - Typing shows suggestions with nothing selected (no highlight).
//! - Down selects the first suggestion, filling the line with its full
//!   text; the selected suggestion — not the typed prefix — is what
//!   actually gets submitted on Enter.
//! - Tab is bash's own native completion when nothing is selected (not our
//!   suggestion box) — this tool doesn't touch Tab at all in that state.
//! - Tab is a no-op once something is selected via Up/Down (Enter is how a
//!   selection gets confirmed instead).
//!
//! Drives a real bash session in a pty (see tests/common) since none of
//! this is observable from generated-script text alone.

mod common;

use common::BashSession;

#[test]
fn selecting_a_suggestion_and_completing_with_tab() {
    let bin = env!("CARGO_BIN_EXE_rsreadline");
    let history = "echo alpha\necho beta\necho gamma\n";
    let session = BashSession::spawn(history);
    session.send_and_drain(format!("eval \"$({bin} init bash)\"\n").as_bytes());
    session.send_and_drain(b"echo hi\n"); // realistic warm-up, not the very first byte

    // --- Phase 1: typing shows suggestions, nothing selected ---
    let mut typed = Vec::new();
    for byte in b"echo" {
        typed = session.send_and_drain(&[*byte]);
    }
    let text = String::from_utf8_lossy(&typed);
    assert!(
        text.contains("echo gamma") && text.contains("echo beta") && text.contains("echo alpha"),
        "expected all three suggestions after typing 'echo':\n{text}"
    );
    assert!(
        !text.contains("\x1b[7m"),
        "nothing should be selected/highlighted right after typing:\n{text}"
    );

    // --- Phase 2: Down selects the first (most recent) suggestion ---
    let after_down = session.send_and_drain(b"\x1b[B");
    let text = String::from_utf8_lossy(&after_down);
    assert!(
        text.contains("\x1b[7mecho gamma\x1b[0m"),
        "expected 'echo gamma' (most recent match) highlighted after Down:\n{text}"
    );
    assert!(
        text.ends_with("echo gamma"),
        "expected the line itself to be filled with the selection:\n{text}"
    );

    // --- Phase 3: Enter submits the SELECTED suggestion, not "echo" ---
    let after_enter = session.send_and_drain(b"\n");
    let text = String::from_utf8_lossy(&after_enter);
    assert!(
        text.contains("gamma"),
        "expected the selected command's real output ('gamma') after Enter:\n{text}"
    );

    // --- Phase 4: fresh prompt, Tab with nothing selected is bash's own
    // native completion, not us. Native completion's exact effect on
    // "echo" is filesystem/environment-dependent, so rather than asserting
    // a specific outcome, assert our tool never got involved at all: native
    // `complete` runs entirely inside readline, not as a shell command, so
    // it never triggers our DEBUG-trap preexec hook — no rsreadline escape
    // sequences should appear at all, and critically the line must NOT
    // become our top suggestion ("echo gamma"), which is what the old,
    // incorrect behavior would have produced.
    for byte in b"echo" {
        session.send_and_drain(&[*byte]);
    }
    let after_tab = session.send_and_drain(b"\t");
    let text = String::from_utf8_lossy(&after_tab);
    assert!(
        !text.contains("\x1b7"),
        "native completion shouldn't trigger any of our rendering at all:\n{text}"
    );
    assert!(
        !text.ends_with("echo gamma"),
        "Tab must not fill in our top suggestion when nothing is selected \
         (that's native bash completion's job now, not ours):\n{text}"
    );

    session.send_and_drain(b"\x03"); // abandon the line, fresh prompt

    // --- Phase 5: fresh prompt, Tab is a no-op once something is selected ---
    for byte in b"echo" {
        session.send_and_drain(&[*byte]);
    }
    session.send_and_drain(b"\x1b[B"); // select "echo gamma"
    let after_tab2 = session.send_and_drain(b"\t");
    let text2 = String::from_utf8_lossy(&after_tab2);
    // __rsreadline_tab_noop calls __rsreadline_update "stay", which redraws
    // the exact same selection rather than actually changing anything — so
    // the highlighted suggestion must still be there, and the line must be
    // unchanged, even though a real render did happen (see ARCHITECTURE.md
    // for why "stay" redraws instead of truly doing nothing at the byte
    // level).
    assert!(
        text2.contains("\x1b[7mecho gamma\x1b[0m"),
        "expected 'echo gamma' to remain selected/highlighted after a no-op Tab:\n{text2}"
    );
    assert!(
        text2.ends_with("echo gamma"),
        "expected the line to remain unchanged by the no-op Tab:\n{text2}"
    );

    // The line must still be the selection, unaffected by the no-op Tab.
    let after_enter2 = session.send_and_drain(b"\n");
    let text = String::from_utf8_lossy(&after_enter2);
    assert!(
        text.contains("gamma"),
        "expected the still-selected command to run after Enter:\n{text}"
    );
}
