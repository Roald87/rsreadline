use std::ffi::OsString;
use std::fs;
use std::io;
use std::path::Path;

pub fn load_entries(path: &Path) -> Vec<String> {
    let text = fs::read_to_string(path).unwrap_or_default();
    parse_entries(&text)
}

/// Removes every occurrence of `target` from the history file at `path`
/// (so a polluting entry stops being suggested for good, not just once),
/// along with each occurrence's paired `HISTTIMEFORMAT` timestamp-comment
/// line, if it has one. No-ops (successfully) if the file doesn't exist or
/// `target` doesn't appear in it.
///
/// Writes atomically (temp file + rename) rather than truncate-in-place:
/// this is the one place rsreadline mutates the user's live shell history
/// rather than just reading it, and a write that's interrupted partway
/// through would otherwise risk turning "delete one line" into "wipe the
/// whole file". The original file's permission bits are preserved on the
/// temp file before the rename, since `.bash_history` can hold secrets
/// typed into commands and a `chmod 600` shouldn't get silently loosened
/// to umask-default permissions.
///
/// Note: rejoining with `\n` after `str::lines()` normalizes any CRLF line
/// endings in the file to LF, even on untouched lines — accepted since
/// this is a Linux-only tool and `.bash_history` is effectively always LF.
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
    use std::path::PathBuf;
    use std::sync::atomic::{AtomicUsize, Ordering};

    static NEXT_ID: AtomicUsize = AtomicUsize::new(0);

    /// Writes `contents` to a uniquely-named file under the system temp dir
    /// so parallel tests don't collide, and returns its path.
    fn temp_history_file(contents: &str) -> PathBuf {
        let path = std::env::temp_dir().join(format!(
            "rsreadline_history_test_{}_{}",
            std::process::id(),
            NEXT_ID.fetch_add(1, Ordering::Relaxed)
        ));
        fs::write(&path, contents).unwrap();
        path
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
