//! Substring matching over history entries: prefix-ranked, deduped,
//! most-recent-first.

use std::collections::HashSet;

/// Returns up to `max` history entries matching `query`, most-relevant first:
/// entries that start with `query` come before entries that merely contain
/// it, and within each group the most recently used entry comes first.
/// Entries are deduped, keeping each string's most recent occurrence.
///
/// Substring, not fuzzy: PSReadline (the tool this suggestion style is
/// modeled on) does the same, since a mid-string match outranking an
/// obvious prefix would read as wrong.
///
/// Matching is case-insensitive unless `case_sensitive` is set.
pub fn suggest(entries: &[String], query: &str, max: usize, case_sensitive: bool) -> Vec<String> {
    if query.is_empty() {
        return Vec::new();
    }

    let query = &if case_sensitive {
        query.to_string()
    } else {
        query.to_lowercase()
    };

    let mut seen: HashSet<&str> = HashSet::new();
    let mut prefix_matches: Vec<String> = Vec::new();
    let mut contains_matches: Vec<String> = Vec::new();

    for entry in entries.iter().rev() {
        if !seen.insert(entry.as_str()) {
            continue;
        }
        let candidate = if case_sensitive {
            entry.clone()
        } else {
            entry.to_lowercase()
        };
        if candidate.starts_with(query.as_str()) {
            prefix_matches.push(entry.clone());
        } else if candidate.contains(query.as_str()) {
            contains_matches.push(entry.clone());
        }
    }

    prefix_matches.extend(contains_matches);
    prefix_matches.truncate(max);
    prefix_matches
}

#[cfg(test)]
mod tests {
    use super::*;

    fn entries(items: &[&str]) -> Vec<String> {
        items.iter().map(|s| s.to_string()).collect()
    }

    #[test]
    fn empty_query_returns_empty() {
        let entries = entries(&["git status", "git commit"]);
        assert!(suggest(&entries, "", 5, false).is_empty());
    }

    #[test]
    fn substring_match_finds_mid_string_hits() {
        let entries = entries(&["cargo build --release", "ls"]);
        assert_eq!(
            suggest(&entries, "build", 5, false),
            vec!["cargo build --release"]
        );
    }

    #[test]
    fn prefix_matches_are_ranked_before_contains_only_matches() {
        let entries = entries(&["cargo build", "build cargo"]);
        // "build cargo" contains "cargo" but doesn't start with it;
        // "cargo build" starts with "cargo".
        assert_eq!(
            suggest(&entries, "cargo", 5, false),
            vec!["cargo build", "build cargo"]
        );
    }

    #[test]
    fn most_recent_first_within_each_group() {
        let entries = entries(&["git status", "git commit", "git status"]);
        // "git status" appears twice; only its most recent occurrence (index 2)
        // should be kept, and it should come after "git commit" was typed later
        // than the first "git status" but before the second.
        assert_eq!(
            suggest(&entries, "git", 5, false),
            vec!["git status", "git commit"]
        );
    }

    #[test]
    fn dedupe_keeps_most_recent_occurrence() {
        let entries = entries(&["ls -la", "cd /tmp", "ls -la"]);
        let result = suggest(&entries, "ls", 5, false);
        assert_eq!(result, vec!["ls -la"]);
    }

    #[test]
    fn capped_at_max() {
        let entries = entries(&["git a", "git b", "git c", "git d"]);
        assert_eq!(suggest(&entries, "git", 2, false).len(), 2);
    }

    #[test]
    fn no_match_returns_empty() {
        let entries = entries(&["ls -la", "cd /tmp"]);
        assert!(suggest(&entries, "docker", 5, false).is_empty());
    }

    #[test]
    fn case_insensitive_by_default() {
        let entries = entries(&["Git Status"]);
        assert_eq!(suggest(&entries, "git", 5, false), vec!["Git Status"]);
    }

    #[test]
    fn case_sensitive_when_enabled() {
        let entries = entries(&["Git Status"]);
        assert!(suggest(&entries, "git", 5, true).is_empty());
        assert_eq!(suggest(&entries, "Git", 5, true), vec!["Git Status"]);
    }
}
