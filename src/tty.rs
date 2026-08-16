use std::fs::OpenOptions;
use std::io::{self, Write};
use std::os::fd::AsRawFd;

const SAVE_CURSOR: &str = "\x1b7";
const RESTORE_CURSOR: &str = "\x1b8";
const CLEAR_LINE: &str = "\x1b[2K\r";
const SELECTED_MARKER: &str = "> ";
const UNSELECTED_MARKER: &str = "  ";
pub const HEIGHT_WARNING: &str = "rsreadline: suggestions hidden (terminal too short)";

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

/// Builds the escape sequence that draws a fixed `block_size`-line suggestion
/// block below the cursor, then restores the cursor to its original position.
/// Slots beyond `lines.len()` are drawn as cleared blank lines.
pub fn render_sequence(lines: &[String], selected: usize, block_size: usize) -> String {
    let mut out = String::from(SAVE_CURSOR);
    for i in 0..block_size {
        out.push('\n');
        out.push_str(CLEAR_LINE);
        if let Some(line) = lines.get(i) {
            let marker = if i == selected {
                SELECTED_MARKER
            } else {
                UNSELECTED_MARKER
            };
            out.push_str(marker);
            out.push_str(line);
        }
    }
    out.push_str(RESTORE_CURSOR);
    out
}

/// Builds the escape sequence that clears a fixed `block_size`-line region
/// below the cursor without writing anything into it.
pub fn clear_sequence(block_size: usize) -> String {
    let mut out = String::from(SAVE_CURSOR);
    for _ in 0..block_size {
        out.push('\n');
        out.push_str(CLEAR_LINE);
    }
    out.push_str(RESTORE_CURSOR);
    out
}

/// Builds the single-line escape sequence shown in place of the block when
/// the terminal is too short for `min_terminal_height`.
pub fn warning_sequence(message: &str) -> String {
    format!("{SAVE_CURSOR}\n{CLEAR_LINE}{message}{RESTORE_CURSOR}")
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
    fn render_sequence_saves_and_restores_cursor() {
        let seq = render_sequence(&[], 0, 5);
        assert!(seq.starts_with(SAVE_CURSOR));
        assert!(seq.ends_with(RESTORE_CURSOR));
    }

    #[test]
    fn render_sequence_pads_missing_slots_as_blank() {
        let lines = vec!["ls -la".to_string()];
        let seq = render_sequence(&lines, 0, 3);
        // one selected line with content, two blank cleared lines
        assert_eq!(seq.matches(CLEAR_LINE).count(), 3);
        assert!(seq.contains("> ls -la"));
    }

    #[test]
    fn render_sequence_marks_selected_line() {
        let lines = vec!["a".to_string(), "b".to_string()];
        let seq = render_sequence(&lines, 1, 2);
        assert!(seq.contains("  a"));
        assert!(seq.contains("> b"));
    }

    #[test]
    fn clear_sequence_has_no_visible_content() {
        let seq = clear_sequence(5);
        assert_eq!(seq.matches(CLEAR_LINE).count(), 5);
        assert!(!seq.contains("ls"));
    }

    #[test]
    fn warning_sequence_contains_the_message() {
        let seq = warning_sequence(HEIGHT_WARNING);
        assert!(seq.starts_with(SAVE_CURSOR));
        assert!(seq.ends_with(RESTORE_CURSOR));
        assert!(seq.contains(HEIGHT_WARNING));
        assert_eq!(seq.matches(CLEAR_LINE).count(), 1);
    }
}
