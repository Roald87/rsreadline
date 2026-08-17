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
        ["complete", line, point, selected] => {
            print!("{}", cmd_complete(&Config::load(), line, point, selected));
            ExitCode::SUCCESS
        }
        _ => {
            eprintln!(
                "usage: rsreadline <init bash | render <line> <point> <selected> <direction> | complete <line> <point> <selected>>"
            );
            ExitCode::FAILURE
        }
    }
}

/// Recomputes matches for the current line, advances the selected index by
/// `direction`, draws the suggestion block (or the height warning) to
/// /dev/tty, and returns "selected_index\tmatch_count" for the bash glue.
fn cmd_render(config: &Config, line: &str, point: &str, selected: &str, direction: &str) -> String {
    let query = line_prefix(line, parse_usize(point));
    let entries = history::load_entries(&config.history_file);
    let matches = matcher::suggest(&entries, query, config.max_suggestions);
    let new_selected = next_selected(parse_usize(selected), matches.len(), direction);

    draw(config, &matches, new_selected);

    format!("{new_selected}\t{}", matches.len())
}

/// Returns the full text of the currently-selected suggestion (or an empty
/// string if there is none) for the bash glue to assign to READLINE_LINE.
fn cmd_complete(config: &Config, line: &str, point: &str, selected: &str) -> String {
    let query = line_prefix(line, parse_usize(point));
    let entries = history::load_entries(&config.history_file);
    let matches = matcher::suggest(&entries, query, config.max_suggestions);
    matches
        .get(parse_usize(selected))
        .cloned()
        .unwrap_or_default()
}

fn draw(config: &Config, matches: &[String], selected: usize) {
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

/// Returns the byte-slice of `line` covering its first `point` characters
/// (clamped to the line's length), UTF-8 safe.
fn line_prefix(line: &str, point: usize) -> &str {
    match line.char_indices().nth(point) {
        Some((byte_idx, _)) => &line[..byte_idx],
        None => line,
    }
}

/// Advances `current` by `direction` ("up"/"down") within `[0, count)`,
/// wrapping at the ends; any other direction (e.g. "none") keeps `current`
/// if still in range, otherwise resets to the top match.
fn next_selected(current: usize, count: usize, direction: &str) -> usize {
    if count == 0 {
        return 0;
    }
    match direction {
        "up" => (current + count - 1) % count,
        "down" => (current + 1) % count,
        _ => {
            if current < count {
                current
            } else {
                0
            }
        }
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
    fn next_selected_wraps_up_and_down() {
        assert_eq!(next_selected(0, 3, "up"), 2);
        assert_eq!(next_selected(2, 3, "down"), 0);
        assert_eq!(next_selected(1, 3, "up"), 0);
        assert_eq!(next_selected(1, 3, "down"), 2);
    }

    #[test]
    fn next_selected_none_keeps_valid_index() {
        assert_eq!(next_selected(2, 5, "none"), 2);
    }

    #[test]
    fn next_selected_none_resets_out_of_range_index() {
        assert_eq!(next_selected(9, 3, "none"), 0);
    }

    #[test]
    fn next_selected_with_no_matches_is_zero() {
        assert_eq!(next_selected(2, 0, "up"), 0);
        assert_eq!(next_selected(2, 0, "down"), 0);
    }
}
