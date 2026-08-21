//! Reads and mutates `.bash_history`: parses entries (skipping
//! `HISTTIMEFORMAT` timestamp lines) and removes a given entry's
//! occurrences on disk.

use std::ffi::OsString;
use std::fs;
use std::io;
use std::path::Path;

pub fn load_entries(path: &Path) -> Vec<String> {
    let text = fs::read_to_string(path).unwrap_or_default();
    parse_entries(&text)
}

/// Removes every occurrence of `target` from the history file at `path`
/// (all occurrences, not just the most recent), along with each
/// occurrence's paired `HISTTIMEFORMAT` timestamp line, if any. No-ops if
/// the file doesn't exist or `target` isn't in it.
///
/// Writes atomically (temp file + rename): this is the one place
/// rsreadline mutates the user's live history, and an interrupted
/// truncate-in-place write could turn "delete one line" into "wipe the
/// whole file". Permission bits are copied onto the temp file first, since
/// `.bash_history` can hold secrets and a `chmod 600` shouldn't get
/// silently loosened.
///
/// Rejoining with `\n` after `str::lines()` normalizes CRLF to LF on every
/// line — accepted, since this is Linux-only and `.bash_history` is
/// effectively always LF.
pub fn remove_entry(path: &Path, target: &str) -> io::Result<()> {
    let Ok(text) = fs::read_to_string(path) else {
        return Ok(());
    };

    let lines: Vec<&str> = text.lines().collect();
    let mut removed = vec![false; lines.len()];
    for (i, line) in lines.iter().enumerate() {
        if !is_timestamp_comment(line) && line.trim_end() == target {
            removed[i] = true;
            if i > 0 && is_timestamp_comment(lines[i - 1]) {
                removed[i - 1] = true;
            }
        }
    }

    if !removed.iter().any(|&r| r) {
        return Ok(());
    }

    let kept: Vec<&str> = lines
        .iter()
        .zip(removed.iter())
        .filter(|&(_, &r)| !r)
        .map(|(&line, _)| line)
        .collect();
    let mut new_text = kept.join("\n");
    if !kept.is_empty() {
        new_text.push('\n');
    }

    write_atomically(path, &new_text)
}

fn write_atomically(path: &Path, contents: &str) -> io::Result<()> {
    let mut file_name = path.file_name().map(OsString::from).unwrap_or_default();
    file_name.push(".rsreadline.tmp");
    let tmp_path = path.with_file_name(file_name);

    let write_result = fs::write(&tmp_path, contents).and_then(|()| {
        if let Ok(metadata) = fs::metadata(path) {
            fs::set_permissions(&tmp_path, metadata.permissions())?;
        }
        fs::rename(&tmp_path, path)
    });

    if write_result.is_err() {
        let _ = fs::remove_file(&tmp_path);
    }
    write_result
}

fn parse_entries(text: &str) -> Vec<String> {
    text.lines()
        .filter(|line| !is_timestamp_comment(line))
        // Normalize away trailing whitespace so dedup treats these as equal.
        .map(|line| line.trim_end().to_string())
        .collect()
}

fn is_timestamp_comment(line: &str) -> bool {
    match line.strip_prefix('#') {
        Some(rest) => !rest.is_empty() && rest.bytes().all(|b| b.is_ascii_digit()),
        None => false,
    }
}

#[cfg(test)]
#[allow(clippy::unwrap_used, clippy::expect_used)]
mod tests {
    use super::*;
    use crate::test_support::temp_history_file as temp_file;

    fn temp_history_file(contents: &str) -> std::path::PathBuf {
        temp_file("history", contents)
    }

    #[test]
    fn remove_entry_removes_all_occurrences() {
        let path = temp_history_file("ls -la\ncd /tmp\nls -la\ngit status\n");
        remove_entry(&path, "ls -la").unwrap();
        assert_eq!(fs::read_to_string(&path).unwrap(), "cd /tmp\ngit status\n");
        fs::remove_file(&path).unwrap();
    }

    #[test]
    fn remove_entry_removes_paired_timestamp_line() {
        let path = temp_history_file(
            "#1700000000\nls -la\n#1700000001\ncd /tmp\nls -la\n#1700000002\ngit status\n",
        );
        remove_entry(&path, "ls -la").unwrap();
        // The first "ls -la" has a timestamp predecessor, so it's removed
        // too. The second "ls -la"'s predecessor is "cd /tmp" (not a
        // timestamp line), so nothing extra is removed for it — and
        // "#1700000001" (paired with the surviving "cd /tmp") must survive.
        assert_eq!(
            fs::read_to_string(&path).unwrap(),
            "#1700000001\ncd /tmp\n#1700000002\ngit status\n"
        );
        fs::remove_file(&path).unwrap();
    }

    #[test]
    fn remove_entry_is_noop_when_target_absent() {
        let path = temp_history_file("cd /tmp\ngit status\n");
        remove_entry(&path, "ls -la").unwrap();
        assert_eq!(fs::read_to_string(&path).unwrap(), "cd /tmp\ngit status\n");
        fs::remove_file(&path).unwrap();
    }

    #[test]
    fn remove_entry_is_noop_on_missing_file() {
        let path = Path::new("/nonexistent/path/that/does/not/exist");
        assert!(remove_entry(path, "ls -la").is_ok());
    }

    #[test]
    fn remove_entry_preserves_permission_bits() {
        use std::os::unix::fs::PermissionsExt;

        let path = temp_history_file("ls -la\ncd /tmp\n");
        fs::set_permissions(&path, fs::Permissions::from_mode(0o600)).unwrap();

        remove_entry(&path, "ls -la").unwrap();

        let mode = fs::metadata(&path).unwrap().permissions().mode() & 0o777;
        assert_eq!(mode, 0o600);
        fs::remove_file(&path).unwrap();
    }

    #[test]
    fn timestamp_lines_are_skipped() {
        let text = "#1700000000\nls -la\n#1700000001\ncd /tmp\n";
        assert_eq!(parse_entries(text), vec!["ls -la", "cd /tmp"]);
    }

    #[test]
    fn non_numeric_hash_prefixed_line_is_kept() {
        let text = "#this is a real comment command\nls\n";
        assert_eq!(
            parse_entries(text),
            vec!["#this is a real comment command", "ls"]
        );
    }

    #[test]
    fn order_is_preserved() {
        let text = "one\ntwo\nthree\n";
        assert_eq!(parse_entries(text), vec!["one", "two", "three"]);
    }

    #[test]
    fn empty_hash_line_is_kept() {
        let text = "#\nls\n";
        assert_eq!(parse_entries(text), vec!["#", "ls"]);
    }

    #[test]
    fn trailing_whitespace_is_trimmed() {
        let text = "bundle exec jekyll serve\nbundle exec jekyll serve \n";
        assert_eq!(
            parse_entries(text),
            vec!["bundle exec jekyll serve", "bundle exec jekyll serve"]
        );
    }

    #[test]
    fn trailing_carriage_return_is_trimmed() {
        let text = "ls -la\r\ncd /tmp\r\n";
        assert_eq!(parse_entries(text), vec!["ls -la", "cd /tmp"]);
    }

    #[test]
    fn missing_file_yields_no_entries() {
        assert_eq!(
            load_entries(Path::new("/nonexistent/path/that/does/not/exist")),
            Vec::<String>::new()
        );
    }
}
