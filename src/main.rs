mod bashgen;
mod config;
mod history;
mod matcher;
mod tty;

use config::Config;
use std::process::ExitCode;

fn main() -> ExitCode {
    let args: Vec<String> = std::env::args().skip(1).collect();
    let refs: Vec<&str> = args.iter().map(String::as_str).collect();

    match refs.as_slice() {
        ["init", "bash"] => {
            print!("{}", bashgen::generate(&Config::load()));
            ExitCode::SUCCESS
        }
        ["render", line, point, selected, direction] => {
            print!(
                "{}",
                cmd_render(&Config::load(), line, point, selected, direction)
            );
            ExitCode::SUCCESS
        }
        _ => {
            eprintln!(
                "usage: rsreadline <init bash | render <line> <point> <selected> <direction>>"
            );
            ExitCode::FAILURE
        }
    }
}

/// Recomputes matches, advances the selection by `direction`, draws the
/// suggestion block (or the height warning) to /dev/tty, and returns
/// "selected_index\x01match_count\x01fill_text" for the bash glue —
/// `selected` is empty when nothing is selected, and `fill_text` (the newly
/// selected suggestion's full text, for the glue to load into
/// READLINE_LINE) is likewise empty unless a real selection was just made.
/// The separator is \x01 (SOH), not a tab: bash's `read` silently drops
/// empty fields anywhere but the last when splitting on IFS whitespace
/// (space/tab/newline) even with a custom IFS, which corrupts `selected`/
/// `fill_text` whenever they're legitimately empty — see the matching
/// comment in bashgen.rs's `__rsreadline_update` for the verified example.
///
/// The bash glue decides *what* `line`/`point` mean here: for a typing
/// event (`direction == "none"`) it passes the current READLINE_LINE, which
/// also becomes the new stored query; for an up/down cycle it passes the
/// stored query back unchanged, so cycling keeps matching against what was
/// actually typed rather than whatever the last selection preview left in
/// READLINE_LINE. See ARCHITECTURE.md ("Selecting a suggestion").
fn cmd_render(config: &Config, line: &str, point: &str, selected: &str, direction: &str) -> String {
    let query = line_prefix(line, parse_usize(point));
    let entries = history::load_entries(&config.history_file);
    let matches = matcher::suggest(&entries, query, config.max_suggestions);
    let new_selected = next_selected(parse_selected(selected), matches.len(), direction);

    draw(config, &matches, new_selected);

    let sel_field = new_selected.map_or(String::new(), |i| i.to_string());
    let fill = new_selected
        .and_then(|i| matches.get(i))
        .cloned()
        .unwrap_or_default();
    format!("{sel_field}\x01{}\x01{fill}", matches.len())
}

fn draw(config: &Config, matches: &[String], selected: Option<usize>) {
    let Some((rows, _cols)) = tty::terminal_size() else {
        return;
    };
    let sequence = if tty::should_render(rows, config.min_terminal_height) {
        tty::render_sequence(matches, selected, config.max_suggestions)
    } else {
        tty::warning_sequence(&tty::height_warning(config.min_terminal_height))
    };
    let _ = tty::write_to_tty(&sequence);
}

fn parse_usize(value: &str) -> usize {
    value.parse().unwrap_or(0)
}

/// Empty string is the "nothing selected" sentinel (see `_RSREADLINE_SEL` in
/// bashgen.rs); anything else is a selected index.
fn parse_selected(value: &str) -> Option<usize> {
    if value.is_empty() {
        None
    } else {
        value.parse().ok()
    }
}

/// Returns the byte-slice of `line` covering its first `point` characters
/// (clamped to the line's length), UTF-8 safe.
fn line_prefix(line: &str, point: usize) -> &str {
    match line.char_indices().nth(point) {
        Some((byte_idx, _)) => &line[..byte_idx],
        None => line,
    }
}

/// Advances the selection by `direction` within `[0, count)`, wrapping at
/// the ends:
/// - "up"/"down": cycle; from "nothing selected" they land on the last/
///   first match respectively.
/// - "stay": keep the current selection exactly as-is. Used to redraw the
///   block without changing anything — e.g. Tab is a no-op once something
///   is selected (Enter is how a selection gets confirmed instead), but
///   still needs to repaint over the DEBUG-trap preexec hook's harmless
///   spurious clear (see ARCHITECTURE.md) rather than leaving the block
///   blank until the next real keystroke.
/// - anything else (typing, e.g. "none"): clears the selection — suggestions
///   appear unselected as you type; only Up/Down picks one.
fn next_selected(current: Option<usize>, count: usize, direction: &str) -> Option<usize> {
    if count == 0 {
        return None;
    }
    match direction {
        "down" => Some(match current {
            None => 0,
            Some(i) => (i + 1) % count,
        }),
        "up" => Some(match current {
            None => count - 1,
            Some(i) => (i + count - 1) % count,
        }),
        "stay" => current,
        _ => None,
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn line_prefix_clamps_to_line_length() {
        assert_eq!(line_prefix("git status", 3), "git");
        assert_eq!(line_prefix("git", 100), "git");
        assert_eq!(line_prefix("git", 0), "");
    }

    #[test]
    fn down_from_nothing_selected_lands_on_first() {
        assert_eq!(next_selected(None, 3, "down"), Some(0));
    }

    #[test]
    fn up_from_nothing_selected_lands_on_last() {
        assert_eq!(next_selected(None, 3, "up"), Some(2));
    }

    #[test]
    fn next_selected_wraps_up_and_down() {
        assert_eq!(next_selected(Some(0), 3, "up"), Some(2));
        assert_eq!(next_selected(Some(2), 3, "down"), Some(0));
        assert_eq!(next_selected(Some(1), 3, "up"), Some(0));
        assert_eq!(next_selected(Some(1), 3, "down"), Some(2));
    }

    #[test]
    fn typing_always_clears_the_selection() {
        assert_eq!(next_selected(Some(2), 5, "none"), None);
        assert_eq!(next_selected(None, 5, "none"), None);
    }

    #[test]
    fn stay_keeps_the_current_selection_unchanged() {
        assert_eq!(next_selected(Some(1), 3, "stay"), Some(1));
        assert_eq!(next_selected(None, 3, "stay"), None);
    }

    #[test]
    fn next_selected_with_no_matches_is_none() {
        assert_eq!(next_selected(Some(2), 0, "up"), None);
        assert_eq!(next_selected(None, 0, "down"), None);
    }

    #[test]
    fn parse_selected_treats_empty_string_as_nothing_selected() {
        assert_eq!(parse_selected(""), None);
        assert_eq!(parse_selected("0"), Some(0));
        assert_eq!(parse_selected("3"), Some(3));
    }
}
