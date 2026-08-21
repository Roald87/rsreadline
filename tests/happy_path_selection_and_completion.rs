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
//!
//! Phase 3/6's Enter checks assert byte-exact structure — our DEBUG-trap
//! clear sequence immediately followed by clean output — rather than a
//! loose `contains("gamma")`. A loose substring check can't tell "gamma"
//! printed cleanly on its own row apart from "gamma" glued onto stale
//! suggestion text or printed over the prompt row: both real-world
//! regressions (see `enter_preexec_clears_stale_suggestions.rs`) still
//! contain the substring "gamma" somewhere in the raw bytes.
//!
//! The expected sequence below is a literal, not built via `tty.rs`
//! (there's no way to reach it from an integration test — this is a
//! binary-only crate) — which also means it can't go tautological the way
//! importing the function under test would: if `tty.rs` regresses, this
//! byte literal doesn't regress with it.

mod common;

use common::BashSession;

/// `tty::preexec_clear_sequence(5)`'s shape, hand-verified against a real
/// pty capture — cursor-up (undo accept-line's own newline), reserve 5
/// rows, clear them, return, cursor-down (land back where accept-line put
/// it). 5 is config's default `max_suggestions`; the fake `$HOME` this
/// test spawns bash in has no `config.toml`, so the binary falls back to
/// it (see `config::DEFAULT_MAX_SUGGESTIONS`).
const PREEXEC_CLEAR_5: &[u8] = b"\x1b[1A\
\x1bD\x1bD\x1bD\x1bD\x1bD\x1b[5A\
\x1b[1B\x1b[2K\r\x1b[1B\x1b[2K\r\x1b[1B\x1b[2K\r\x1b[1B\x1b[2K\r\x1b[1B\x1b[2K\r\x1b[5A\
\x1b[1B";

/// Asserts `after_enter` contains our DEBUG-trap clear sequence
/// immediately followed by `expected_output` — i.e. the submitted
/// command's output landed cleanly on its own row, not glued onto or
/// overwriting stale block/prompt content.
fn assert_clean_output_after_enter(after_enter: &[u8], expected_output: &str) {
    let mut expected_tail = PREEXEC_CLEAR_5.to_vec();
    expected_tail.extend_from_slice(expected_output.as_bytes());
    let text = String::from_utf8_lossy(after_enter);
    assert!(
        after_enter
            .windows(expected_tail.len())
            .any(|w| w == expected_tail.as_slice()),
        "expected our clear sequence immediately followed by clean {expected_output:?} output, \
         found neither (or something glued in between) in:\n{text:?}"
    );
}

#[test]
fn selecting_a_suggestion_and_completing_with_tab() {
    let bin = env!("CARGO_BIN_EXE_rsreadline");
    let history = "echo alpha\necho beta\necho gamma\n";
    let session = BashSession::spawn(history);
    session.send_and_drain(format!("eval \"$({bin} init bash)\"\n").as_bytes());
    // Realistic warm-up, not the very first byte. Deliberately not an
    // "echo"-prefixed command: `history -a` in __rsreadline_prompt_reset
    // flushes it to HISTFILE right after it runs, so it would otherwise
    // become a real (and, being most recent, top-ranked) match for the
    // "echo" queries below.
    session.send_and_drain(b"true\n");

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
    assert_clean_output_after_enter(&after_enter, "gamma\r\n");

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
    assert_clean_output_after_enter(&after_enter2, "gamma\r\n");
}
