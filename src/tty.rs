//! Terminal size (via `ioctl`/`TIOCGWINSZ`) and the raw ANSI escape
//! sequences used to draw/clear the suggestion block.

use std::fs::OpenOptions;
use std::io::{self, Write};
use std::os::fd::AsRawFd;

const SAVE_CURSOR: &str = "\x1b7";
const RESTORE_CURSOR: &str = "\x1b8";
// Cursor Down (CUD), not a bare "\n": CUD clamps at the bottom row instead
// of scrolling, so it can't invalidate an absolute saved cursor position
// the way a scrolling linefeed would. Used by render_sequence/
// warning_sequence, which run mid-keystroke while bash's readline is doing
// its own incremental, diff-based redraws from a screen model that knows
// nothing about our writes — an actual scroll would shift real content out
// from under that model, corrupting the next redraw (observed as the
// prompt line growing stray padding). So this path must never scroll — see
// `reserve_rows` for the one place that's safe to.
const CURSOR_DOWN_1: &str = "\x1b[1B";
const CLEAR_LINE: &str = "\x1b[2K\r";
// Index (IND): moves down one row, scrolling if the cursor is on the
// bottom row — like a bare "\n", but as an escape sequence rather than
// byte 0x0A, so the terminal's ONLCR setting can't turn it into CR+LF and
// reset the column. Only used by `clear_sequence`, called at prompt
// boundaries (PROMPT_COMMAND, DEBUG-trap preexec) where readline isn't
// mid-redisplay, so there's no stale on-screen model for a scroll to
// invalidate.
const INDEX_DOWN_1: &str = "\x1bD";
// Reverse video rather than a fixed color: it inverts whatever foreground/
// background the user's own terminal theme already has, so it can't clash.
const REVERSE_VIDEO_START: &str = "\x1b[7m";
const REVERSE_VIDEO_END: &str = "\x1b[0m";
pub fn height_warning(min_height: u16) -> String {
    format!("rsreadline: hidden, need {min_height} rows")
}

/// Linux TIOCGWINSZ ioctl request number (stable across x86_64/aarch64).
const TIOCGWINSZ: u64 = 0x5413;

#[repr(C)]
#[derive(Default)]
struct Winsize {
    ws_row: u16,
    ws_col: u16,
    ws_xpixel: u16,
    ws_ypixel: u16,
}

unsafe extern "C" {
    fn ioctl(fd: i32, request: u64, argp: *mut Winsize) -> i32;
}

/// Queries the controlling terminal's size directly via `/dev/tty`, independent
/// of whatever stdin/stdout happen to be for this process invocation.
pub fn terminal_size() -> Option<(u16, u16)> {
    let tty = OpenOptions::new().read(true).open("/dev/tty").ok()?;
    let mut ws = Winsize::default();
    // SAFETY: `ws` is a valid, correctly-sized `Winsize` for the duration of
    // the call, and `tty`'s fd is valid for the duration of this function.
    let ret = unsafe { ioctl(tty.as_raw_fd(), TIOCGWINSZ, &mut ws) };
    if ret == 0 && ws.ws_row > 0 && ws.ws_col > 0 {
        Some((ws.ws_row, ws.ws_col))
    } else {
        None
    }
}

pub fn should_render(rows: u16, min_height: u16) -> bool {
    rows >= min_height
}

/// Guarantees `rows` blank rows exist below the cursor's current row
/// (forcing a scroll if near the bottom margin), then returns to that row
/// and column. Uses relative movement rather than `SAVE_CURSOR`/
/// `RESTORE_CURSOR`: a scroll carries the cursor along with the content, so
/// undoing our own movement lands back correctly either way.
fn reserve_rows(rows: usize) -> String {
    let mut out = String::new();
    for _ in 0..rows {
        out.push_str(INDEX_DOWN_1);
    }
    out.push_str(&return_to_start(rows));
    out
}

/// Moves the cursor back up `rows` rows — see `reserve_rows`.
fn return_to_start(rows: usize) -> String {
    if rows == 0 {
        String::new()
    } else {
        format!("\x1b[{rows}A")
    }
}

/// Draws a fixed `block_size`-line suggestion block below the cursor, then
/// restores the cursor. Slots beyond `lines.len()` are blank. `selected ==
/// None` highlights nothing.
///
/// Never reserves rows like `clear_sequence` does — runs mid-keystroke,
/// where a forced scroll would corrupt bash's redisplay (see
/// `CURSOR_DOWN_1`). By the time typing starts, `clear_sequence` has
/// already reserved room, so the clamp shouldn't actually bind here.
pub fn render_sequence(lines: &[String], selected: Option<usize>, block_size: usize) -> String {
    let mut out = String::from(SAVE_CURSOR);
    for i in 0..block_size {
        out.push_str(CURSOR_DOWN_1);
        out.push_str(CLEAR_LINE);
        if let Some(line) = lines.get(i) {
            if selected == Some(i) {
                out.push_str(REVERSE_VIDEO_START);
                out.push_str(line);
                out.push_str(REVERSE_VIDEO_END);
            } else {
                out.push_str(line);
            }
        }
    }
    out.push_str(RESTORE_CURSOR);
    out
}

/// Builds the escape sequence that clears a fixed `block_size`-line region
/// below the cursor without writing anything into it.
pub fn clear_sequence(block_size: usize) -> String {
    let mut out = reserve_rows(block_size);
    for _ in 0..block_size {
        out.push_str(CURSOR_DOWN_1);
        out.push_str(CLEAR_LINE);
    }
    out.push_str(&return_to_start(block_size));
    out
}

/// Shown in place of the block when the terminal is shorter than
/// `min_terminal_height`. Mid-edit like `render_sequence` — no row
/// reservation here either.
pub fn warning_sequence(message: &str) -> String {
    format!("{SAVE_CURSOR}{CURSOR_DOWN_1}{CLEAR_LINE}{message}{RESTORE_CURSOR}")
}

pub fn write_to_tty(sequence: &str) -> io::Result<()> {
    let mut tty = OpenOptions::new().write(true).open("/dev/tty")?;
    tty.write_all(sequence.as_bytes())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn should_render_gates_on_min_height() {
        assert!(!should_render(14, 15));
        assert!(should_render(15, 15));
        assert!(should_render(20, 15));
    }

    #[test]
    fn render_sequence_never_reserves_rows() {
        // render_sequence runs on every keystroke, mid-line-edit — it must
        // never force a scroll (via INDEX_DOWN_1/reserve_rows), only the
        // clamped CURSOR_DOWN_1, or it corrupts bash's own incremental
        // redisplay bookkeeping. See CURSOR_DOWN_1's doc comment; confirmed
        // against a real interactive session, not just reasoned about — a
        // scroll during live typing showed up as the prompt line growing
        // stray leading padding.
        let seq = render_sequence(&[], None, 5);
        assert!(
            !seq.contains(INDEX_DOWN_1),
            "must not force a scroll:\n{seq:?}"
        );
        assert!(seq.starts_with(SAVE_CURSOR));
        assert!(seq.ends_with(RESTORE_CURSOR));
    }

    #[test]
    fn render_sequence_pads_missing_slots_as_blank() {
        let lines = vec!["ls -la".to_string()];
        let seq = render_sequence(&lines, Some(0), 3);
        // one selected line with content, two blank cleared lines
        assert_eq!(seq.matches(CLEAR_LINE).count(), 3);
        assert!(seq.contains(&format!("{REVERSE_VIDEO_START}ls -la{REVERSE_VIDEO_END}")));
    }

    #[test]
    fn render_sequence_marks_selected_line() {
        let lines = vec!["a".to_string(), "b".to_string()];
        let seq = render_sequence(&lines, Some(1), 2);
        assert!(!seq.contains(&format!("{REVERSE_VIDEO_START}a")));
        assert!(seq.contains(&format!("{REVERSE_VIDEO_START}b{REVERSE_VIDEO_END}")));
    }

    #[test]
    fn render_sequence_highlights_nothing_when_unselected() {
        let lines = vec!["a".to_string(), "b".to_string()];
        let seq = render_sequence(&lines, None, 2);
        assert!(!seq.contains(REVERSE_VIDEO_START));
        assert!(seq.contains('a'));
        assert!(seq.contains('b'));
    }

    #[test]
    fn clear_sequence_has_no_visible_content() {
        let seq = clear_sequence(5);
        assert_eq!(seq.matches(CLEAR_LINE).count(), 5);
        assert!(!seq.contains("ls"));
    }

    #[test]
    fn clear_sequence_also_reserves_rows_before_clearing_them() {
        let seq = clear_sequence(3);
        assert_eq!(seq.matches(INDEX_DOWN_1).count(), 3);
        assert!(seq.starts_with(&format!("{}{}", INDEX_DOWN_1.repeat(3), "\x1b[3A")));
        assert!(seq.ends_with("\x1b[3A"));
    }

    #[test]
    fn warning_sequence_contains_the_message() {
        let message = height_warning(15);
        let seq = warning_sequence(&message);
        assert!(
            !seq.contains(INDEX_DOWN_1),
            "must not force a scroll:\n{seq:?}"
        );
        assert!(seq.starts_with(SAVE_CURSOR));
        assert!(seq.ends_with(RESTORE_CURSOR));
        assert!(seq.contains(&message));
        assert_eq!(seq.matches(CLEAR_LINE).count(), 1);
    }

    #[test]
    fn height_warning_includes_the_configured_minimum() {
        assert!(height_warning(15).contains("15"));
        assert!(height_warning(20).contains("20"));
    }

    // A bare '\n' scrolls the screen when the cursor is already on the
    // bottom row, which silently invalidates SAVE_CURSOR/RESTORE_CURSOR's
    // saved position — see CURSOR_DOWN_1's doc comment. None of our
    // sequences may use one for vertical movement.
    #[test]
    fn no_sequence_uses_a_bare_linefeed_to_move_down() {
        assert!(!render_sequence(&["x".to_string()], Some(0), 3).contains('\n'));
        assert!(!clear_sequence(3).contains('\n'));
        assert!(!warning_sequence(&height_warning(15)).contains('\n'));
    }
}
