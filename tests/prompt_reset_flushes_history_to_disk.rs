//! Regression test: suggestions are matched against `history_file` on disk
//! (see `history::load_entries`), but bash only appends to HISTFILE on
//! shell exit by default — a command run earlier *this session* wouldn't
//! suggest itself when re-typed, since it never made it to the file. Fixed
//! by `history -a` in `__rsreadline_prompt_reset` (runs via PROMPT_COMMAND
//! right after each command, before the next prompt), which flushes new
//! in-memory history to HISTFILE immediately.
//!
//! Drives a real bash session in a pty (see tests/common) since this is
//! about bash's actual HISTFILE-flushing behavior, not something a unit
//! test against static text can observe.

mod common;

use common::BashSession;

#[test]
fn a_freshly_run_command_suggests_itself_when_retyped() {
    let bin = env!("CARGO_BIN_EXE_rsreadline");
    // Deliberately empty: "echo FRESHMARKER" must not be a pre-seeded
    // suggestion, only one bash just ran in this session.
    let session = BashSession::spawn("");
    session.send_and_drain(format!("eval \"$({bin} init bash)\"\n").as_bytes());

    session.send_and_drain(b"echo FRESHMARKER\n");

    // Retype only a strict prefix, not the full command: the suggestion
    // box's rendered "MARKER" tail can only come from a real history match,
    // not from ordinary keystroke echo (which the earlier `send_and_drain`
    // calls already drained away byte by byte).
    let mut typed = Vec::new();
    for byte in b"echo FRESH" {
        typed = session.send_and_drain(&[*byte]);
    }
    let text = String::from_utf8_lossy(&typed);
    assert!(
        text.contains("echo FRESHMARKER"),
        "expected the just-run command to suggest itself once its prefix is \
         retyped (requires `history -a` to have flushed it to HISTFILE \
         first):\n{text}"
    );
}
