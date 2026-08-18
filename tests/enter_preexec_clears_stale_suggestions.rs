//! Regression test for the Enter/DEBUG-trap preexec bug: without it, the
//! suggestion block sits on screen untouched while a submitted command
//! runs, so short command output can come out with leftover suggestion
//! characters glued onto it (e.g. a real error ending in stray "ve --").
//! See ARCHITECTURE.md ("Enter and the DEBUG-trap preexec hook").
//!
//! The corruption itself is a terminal *grid* artifact (stale characters
//! left in cells an old, wider write touched that a new, shorter write
//! doesn't reach) — it doesn't show up as a literal corrupted substring in
//! the raw byte stream we capture, since nothing re-transmits the stale
//! text during the broken window. What we *can* check at the byte level is
//! ordering: our clear sequence (starting with Index, ESC D — see
//! `reserve_rows` in tty.rs) must be written before the submitted command's
//! own output, not after.

mod common;

use common::BashSession;

#[test]
fn preexec_clears_the_block_before_command_output_not_after() {
    let bin = env!("CARGO_BIN_EXE_rsreadline");
    // A long decoy that "echo MARKER123" is a genuine prefix of, so the
    // suggestion box stays populated right up until Enter is pressed.
    let history = "echo MARKER123_EXTRA_LONG_STALE_SUGGESTION_TAIL_TEXT\n";
    let session = BashSession::spawn(history);

    session.send_and_drain(format!("eval \"$({bin} init bash)\"\n").as_bytes());
    session.send_and_drain(b"echo hi\n");

    for byte in b"echo MARKER123" {
        session.send_and_drain(&[*byte]);
    }

    let after_enter = session.send_and_drain(b"\n");
    let text = String::from_utf8_lossy(&after_enter);

    let clear_pos = after_enter.windows(2).position(|w| w == b"\x1bD");
    let output_pos = text.find("MARKER123");

    assert!(
        output_pos.is_some(),
        "expected the submitted command's real output in:\n{text:?}"
    );
    assert!(
        clear_pos.is_some(),
        "expected our clear sequence (ESC D) somewhere in the response:\n{text:?}"
    );
    assert!(
        clear_pos < output_pos,
        "clear sequence came AFTER the command's output instead of before — \
         the suggestion block was stale while the command ran:\n{text:?}"
    );
}
