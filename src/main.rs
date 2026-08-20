//! Subcommand dispatch for the `rsreadline` binary: `init bash` (see
//! `bashgen`) and `render` (see `cmd_render`).

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
/// The separator is `bashgen::FIELD_SEP` (SOH, not a tab) — see its doc
/// comment for why plain IFS whitespace doesn't work here.
///
/// The bash glue decides *what* `line`/`point` mean here: for a typing
/// event (`direction == "none"`) it passes the current READLINE_LINE, which
/// also becomes the new stored query; for an up/down cycle it passes the
/// stored query back unchanged, so cycling keeps matching against what was
/// actually typed rather than whatever the last selection preview left in
/// READLINE_LINE (see `_RSREADLINE_QUERY` in bashgen.rs).
///
/// `direction == "delete"` is the one case with a disk side effect: if a
/// suggestion is currently selected, its underlying history entry is
/// removed from `history_file` (all occurrences) before matches are
/// recomputed, so the response already reflects the post-delete state in
/// this single round trip. Reuses this same function rather than a separate
/// subcommand: a delete is just "current state + a direction -> new state"
/// like every other direction, and a dedicated subcommand would still need
/// a second `render` call right after it for the redraw anyway.
fn cmd_render(config: &Config, line: &str, point: &str, selected: &str, direction: &str) -> String {
    let query = line_prefix(line, parse_usize(point));
    let mut entries = history::load_entries(&config.history_file);
    let mut matches = matcher::suggest(&entries, query, config.max_suggestions);
    let current = parse_selected(selected);

    if direction == "delete"
        && let Some(target) = current.and_then(|i| matches.get(i))
    {
        let _ = history::remove_entry(&config.history_file, target);
        entries = history::load_entries(&config.history_file);
        matches = matcher::suggest(&entries, query, config.max_suggestions);
    }

    let new_selected = next_selected(current, matches.len(), direction);

    draw(config, &matches, new_selected);

    let sel_field = new_selected.map_or(String::new(), |i| i.to_string());
    let fill = new_selected
        .and_then(|i| matches.get(i))
        .cloned()
        .unwrap_or_default();
    let sep = bashgen::FIELD_SEP;
    format!("{sel_field}{sep}{}{sep}{fill}", matches.len())
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
///   spurious clear (see bashgen.rs's `tab_noop_handler`) rather than leaving the block
///   blank until the next real keystroke.
/// - "delete": like "stay", but clamped into the (possibly shrunk) new
///   `count` — the one direction where the match list can legitimately be
///   a different length than when `current` was chosen, since the caller
///   just removed the selected entry from history. Deliberately distinct
///   from "stay", which existing callers rely on as an exact no-op.
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
        "delete" => current.map(|i| i.min(count - 1)),
        _ => None,
    }
}

#[cfg(test)]
#[allow(clippy::unwrap_used, clippy::expect_used)]
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

    #[test]
    fn delete_keeps_index_when_still_in_bounds() {
        assert_eq!(next_selected(Some(1), 3, "delete"), Some(1));
    }

    #[test]
    fn delete_clamps_to_new_last_index_when_out_of_bounds() {
        assert_eq!(next_selected(Some(2), 2, "delete"), Some(1));
    }

    #[test]
    fn delete_is_none_when_nothing_was_selected() {
        assert_eq!(next_selected(None, 3, "delete"), None);
    }

    fn temp_history_file(contents: &str) -> std::path::PathBuf {
        use std::sync::atomic::{AtomicUsize, Ordering};
        static NEXT_ID: AtomicUsize = AtomicUsize::new(0);
        let path = std::env::temp_dir().join(format!(
            "rsreadline_main_test_{}_{}",
            std::process::id(),
            NEXT_ID.fetch_add(1, Ordering::Relaxed)
        ));
        std::fs::write(&path, contents).unwrap();
        path
    }

    fn test_config(history_file: std::path::PathBuf) -> Config {
        Config {
            history_file,
            max_suggestions: 5,
            min_terminal_height: 15,
        }
    }

    #[test]
    fn cmd_render_delete_removes_selected_entry_from_disk() {
        let path = temp_history_file("git status\ngit commit\ngit push\n");
        let config = test_config(path.clone());

        // Matches are most-recent-first, so index 0 is "git push".
        let result = cmd_render(&config, "git", "3", "0", "delete");

        assert_eq!(
            std::fs::read_to_string(&path).unwrap(),
            "git status\ngit commit\n"
        );
        // Same slot now holds whatever backfilled it.
        let (sel, count, fill) = split_render_result(&result);
        assert_eq!(sel, "0");
        assert_eq!(count, "2");
        assert_eq!(fill, "git commit");

        std::fs::remove_file(&path).unwrap();
    }

    #[test]
    fn cmd_render_delete_with_nothing_selected_does_not_touch_disk() {
        let path = temp_history_file("git status\ngit commit\n");
        let config = test_config(path.clone());

        cmd_render(&config, "git", "3", "", "delete");

        assert_eq!(
            std::fs::read_to_string(&path).unwrap(),
            "git status\ngit commit\n"
        );

        std::fs::remove_file(&path).unwrap();
    }

    fn split_render_result(result: &str) -> (&str, &str, &str) {
        let mut parts = result.split(bashgen::FIELD_SEP);
        let sel = parts.next().unwrap();
        let count = parts.next().unwrap();
        let fill = parts.next().unwrap();
        (sel, count, fill)
    }
}
