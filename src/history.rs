use std::fs;
use std::path::Path;

pub fn load_entries(path: &Path) -> Vec<String> {
    let text = fs::read_to_string(path).unwrap_or_default();
    parse_entries(&text)
}

fn parse_entries(text: &str) -> Vec<String> {
    text.lines()
        .filter(|line| !is_timestamp_comment(line))
        .map(str::to_string)
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
    fn missing_file_yields_no_entries() {
        assert_eq!(
            load_entries(Path::new("/nonexistent/path/that/does/not/exist")),
            Vec::<String>::new()
        );
    }
}
