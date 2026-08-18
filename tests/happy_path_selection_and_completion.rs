//! End-to-end happy-path test for the selection model:
//!
//! - Typing shows suggestions with nothing selected (no highlight).
//! - Down selects the first suggestion, filling the line with its full
//!   text; the selected suggestion — not the typed prefix — is what
//!   actually gets submitted on Enter.
//! - Tab completes to the top match when nothing is selected ("gi<tab>" ->
//!   the top "gi*" match, same as before this feature).
//! - Tab is a no-op once something is selected via Up/Down.
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

    // --- Phase 4: fresh prompt, Tab with nothing selected completes to the top match ---
    for byte in b"echo" {
        session.send_and_drain(&[*byte]);
    }
    let after_tab = session.send_and_drain(b"\t");
    let text = String::from_utf8_lossy(&after_tab);
    assert!(
        text.ends_with("echo gamma"),
        "expected Tab to complete to the top match with nothing selected:\n{text}"
    );
    assert!(
        !text.contains("\x1b[7m"),
        "Tab-completing shouldn't leave anything selected/highlighted:\n{text}"
    );

    session.send_and_drain(b"\x03"); // abandon the line, fresh prompt

    // --- Phase 5: fresh prompt, Tab is a no-op once something is selected ---
    for byte in b"echo" {
        session.send_and_drain(&[*byte]);
    }
    session.send_and_drain(b"\x1b[B"); // select "echo gamma"
    let after_tab2 = session.send_and_drain(b"\t");
    let text2 = String::from_utf8_lossy(&after_tab2);
    // Exactly one SAVE_CURSOR (\x1b7) is expected: the harmless spurious
    // preexec clear that fires before __rsreadline_tab's very first
    // statement, before _RSREADLINE_BUSY is set (see ARCHITECTURE.md). A
    // second one would mean a real __rsreadline_update call happened —
    // i.e. Tab did NOT actually no-op, even if the visible text looks
    // unchanged (e.g. it silently reset the selection).
    let clears = text2.matches("\x1b7").count();
    assert_eq!(
        clears, 1,
        "expected exactly one (harmless, spurious) clear, got {clears} — \
         Tab must not trigger a real re-render once something is selected:\n{text2}"
    );
    // The line must still be the selection, unaffected by the no-op Tab.
    let after_enter2 = session.send_and_drain(b"\n");
    let text = String::from_utf8_lossy(&after_enter2);
    assert!(
        text.contains("gamma"),
        "expected the still-selected command to run after Enter:\n{text}"
    );
}
