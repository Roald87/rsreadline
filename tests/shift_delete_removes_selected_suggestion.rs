//! End-to-end test for Shift+Delete removing a selected suggestion from
//! history entirely:
//!
//! - All occurrences of the deleted command are removed from
//!   `.bash_history` on disk, not just the most recently written one.
//! - If another match survives, it slides into the vacated selected slot
//!   (the block doesn't just go empty).
//! - If the deleted match was the only one, the selection clears and the
//!   line reverts to the bare typed query, not stale preview text.
//! - A history rewrite mid-session doesn't confuse the later `history -a`
//!   flush in PROMPT_COMMAND: a freshly run command still gets flushed and
//!   suggests itself correctly, and deleted entries don't resurrect.
//!
//! Drives a real bash session in a pty (see tests/common) since none of
//! this is observable from generated-script text alone.

#![allow(clippy::expect_used)]

mod common;

use common::BashSession;

#[test]
fn shift_delete_removes_selected_suggestion_from_history() {
    let bin = env!("CARGO_BIN_EXE_rsreadline");
    // "git status" appears twice, "git commit" once — deleting the
    // selected suggestion must remove BOTH "git status" lines, not just
    // the most recently written one.
    let history = "git status\ngit commit\ngit status\n";
    let session = BashSession::spawn(history);
    session.send_and_drain(format!("eval \"$({bin} init bash)\"\n").as_bytes());
    session.send_and_drain(b"true\n"); // realistic warm-up

    // --- Phase 1: type "git", select the top (most recent) match ---
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
        text.contains("\x1b[7mgit status\x1b[0m"),
        "expected 'git status' (most recent match) highlighted after Down:\n{text}"
    );

    // --- Phase 2: Shift+Delete removes it; the OTHER match slides into
    // the vacated selected slot rather than the block just going empty ---
    let after_delete = session.send_and_drain(b"\x1b[3;2~");
    let text = String::from_utf8_lossy(&after_delete);
    assert!(
        !text.contains("git status"),
        "deleted suggestion must not still be shown:\n{text}"
    );
    assert!(
        text.contains("\x1b[7mgit commit\x1b[0m"),
        "expected 'git commit' to slide into the vacated selected slot:\n{text}"
    );
    assert!(
        text.ends_with("git commit"),
        "expected the line to be filled with the newly selected match:\n{text}"
    );

    let on_disk = std::fs::read_to_string(session.fake_home.join(".bash_history"))
        .expect("read .bash_history");
    assert!(
        !on_disk.contains("git status"),
        "expected every occurrence of the deleted entry to be removed from disk:\n{on_disk}"
    );
    assert!(
        on_disk.contains("git commit"),
        "expected the surviving entry to remain on disk:\n{on_disk}"
    );

    // --- Phase 3: delete the last remaining match too — selection clears,
    // and the line reverts to the bare typed query, not stale preview text ---
    let after_second_delete = session.send_and_drain(b"\x1b[3;2~");
    let text = String::from_utf8_lossy(&after_second_delete);
    assert!(
        !text.contains("\x1b[7m"),
        "nothing should remain selected once the last match is deleted:\n{text}"
    );
    assert!(
        text.ends_with("git"),
        "expected the line to revert to the bare typed query, not stale preview text:\n{text}"
    );

    let on_disk = std::fs::read_to_string(session.fake_home.join(".bash_history"))
        .expect("read .bash_history");
    assert!(
        !on_disk.contains("git commit"),
        "expected the second deleted entry to be gone too:\n{on_disk}"
    );

    session.send_and_drain(b"\x03"); // abandon the line, fresh prompt

    // --- Phase 4: a mid-session history rewrite must not confuse the
    // later `history -a` flush in PROMPT_COMMAND — a freshly run command
    // must still get flushed to disk and suggest itself correctly, and the
    // earlier deletes must not have resurrected.
    session.send_and_drain(b"echo FRESHMARKER\n");
    let mut typed = Vec::new();
    for byte in b"echo FRESH" {
        typed = session.send_and_drain(&[*byte]);
    }
    let text = String::from_utf8_lossy(&typed);
    assert!(
        text.contains("echo FRESHMARKER"),
        "expected the freshly run command to still suggest itself after an \
         earlier mid-session history rewrite:\n{text}"
    );

    let on_disk = std::fs::read_to_string(session.fake_home.join(".bash_history"))
        .expect("read .bash_history");
    assert!(
        !on_disk.contains("git status") && !on_disk.contains("git commit"),
        "deleted entries must not have resurrected via a later `history -a` flush:\n{on_disk}"
    );
}
