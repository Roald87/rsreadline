//! Typing mid-line shows matches for the text before the cursor; Up/Down
//! must then cycle that same list, not matches for the whole line.

mod common;

use common::BashSession;

#[test]
fn cycling_after_mid_line_edit_uses_displayed_matches() {
    let bin = env!("CARGO_BIN_EXE_rsreadline");
    let session = BashSession::spawn("echo alpha\necho beta\n");
    session.send_and_drain(format!("eval \"$({bin} init bash)\"\n").as_bytes());
    session.send_and_drain(b"true\n");

    // "eX", Left, "cho" -> line "echoX", cursor before "X", query "echo".
    for byte in b"eX" {
        session.send_and_drain(&[*byte]);
    }
    session.send_and_drain(b"\x1b[D");
    let mut typed = Vec::new();
    for byte in b"cho" {
        typed = session.send_and_drain(&[*byte]);
    }
    let text = String::from_utf8_lossy(&typed);
    assert!(
        text.contains("echo beta"),
        "expected suggestions for 'echo':\n{text}"
    );

    let after_down = session.send_and_drain(b"\x1b[B");
    let text = String::from_utf8_lossy(&after_down);
    assert!(
        text.contains("\x1b[7mecho beta\x1b[0m") && text.ends_with("echo beta"),
        "Down should select the first displayed suggestion:\n{text}"
    );
}
