use std::fs;
use std::path::Path;

pub fn load_entries(path: &Path) -> Vec<String> {
    let text = fs::read_to_string(path).unwrap_or_default();
    parse_entries(&text)
}

fn parse_entries(text: &str) -> Vec<String> {
    text.lines()
        .filter(|line| !is_timestamp_comment(line))
        // Trailing whitespace (e.g. a stray space typed before Enter, or a
        // CRLF-terminated history file leaving a \r) makes two otherwise
        // identical commands look like distinct entries to matcher::suggest's
        // exact-string dedup, showing up as visually-duplicate suggestions.
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
mod tests {
    use super::*;

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
        // A command run once cleanly and once with a stray trailing space
        // typed before Enter must normalize to the same entry, otherwise
        // matcher::suggest's exact-string dedup treats them as distinct and
        // shows what looks like the same suggestion twice.
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
