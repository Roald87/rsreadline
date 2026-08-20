//! Hand-rolled `key = value` config parser for
//! `~/.config/rsreadline/config.toml`.

use std::env;
use std::fs;
use std::path::{Path, PathBuf};

const DEFAULT_HISTORY_FILE: &str = "~/.bash_history";
const DEFAULT_MAX_SUGGESTIONS: usize = 5;
const DEFAULT_MIN_TERMINAL_HEIGHT: u16 = 15;

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Config {
    pub history_file: PathBuf,
    pub max_suggestions: usize,
    pub min_terminal_height: u16,
}

impl Config {
    pub fn load() -> Self {
        let home = home_dir();
        let text = config_path(home.as_deref())
            .and_then(|path| fs::read_to_string(path).ok())
            .unwrap_or_default();
        Self::parse(&text, home.as_deref())
    }

    pub(crate) fn parse(text: &str, home: Option<&Path>) -> Self {
        let mut cfg = Self::defaults(home);
        for line in text.lines() {
            let line = line.trim();
            if line.is_empty() || line.starts_with('#') {
                continue;
            }
            let Some((key, value)) = line.split_once('=') else {
                continue;
            };
            let key = key.trim();
            let value = value.trim().trim_matches('"');
            match key {
                "history_file" => cfg.history_file = expand_tilde(value, home),
                "max_suggestions" => {
                    if let Ok(n) = value.parse() {
                        cfg.max_suggestions = n;
                    }
                }
                "min_terminal_height" => {
                    if let Ok(n) = value.parse() {
                        cfg.min_terminal_height = n;
                    }
                }
                _ => {}
            }
        }
        cfg
    }

    fn defaults(home: Option<&Path>) -> Self {
        Self {
            history_file: expand_tilde(DEFAULT_HISTORY_FILE, home),
            max_suggestions: DEFAULT_MAX_SUGGESTIONS,
            min_terminal_height: DEFAULT_MIN_TERMINAL_HEIGHT,
        }
    }
}

fn home_dir() -> Option<PathBuf> {
    env::var_os("HOME").map(PathBuf::from)
}

fn config_path(home: Option<&Path>) -> Option<PathBuf> {
    home.map(|h| h.join(".config/rsreadline/config.toml"))
}

fn expand_tilde(value: &str, home: Option<&Path>) -> PathBuf {
    if let Some(rest) = value.strip_prefix("~/")
        && let Some(home) = home
    {
        return home.join(rest);
    } else if value == "~"
        && let Some(home) = home
    {
        return home.to_path_buf();
    }
    PathBuf::from(value)
}

#[cfg(test)]
mod tests {
    use super::*;

    fn home() -> PathBuf {
        PathBuf::from("/home/testuser")
    }

    #[test]
    fn defaults_when_file_missing() {
        let cfg = Config::parse("", Some(&home()));
        assert_eq!(cfg.history_file, home().join(".bash_history"));
        assert_eq!(cfg.max_suggestions, 5);
        assert_eq!(cfg.min_terminal_height, 15);
    }

    #[test]
    fn all_keys_set() {
        let text =
            "history_file = ~/custom_history\nmax_suggestions = 3\nmin_terminal_height = 20\n";
        let cfg = Config::parse(text, Some(&home()));
        assert_eq!(cfg.history_file, home().join("custom_history"));
        assert_eq!(cfg.max_suggestions, 3);
        assert_eq!(cfg.min_terminal_height, 20);
    }

    #[test]
    fn partial_file_falls_back_to_defaults_per_key() {
        let text = "max_suggestions = 7\n";
        let cfg = Config::parse(text, Some(&home()));
        assert_eq!(cfg.history_file, home().join(".bash_history"));
        assert_eq!(cfg.max_suggestions, 7);
        assert_eq!(cfg.min_terminal_height, 15);
    }

    #[test]
    fn comments_and_blank_lines_ignored() {
        let text = "# a comment\n\n   \nmax_suggestions = 9\n# trailing comment\n";
        let cfg = Config::parse(text, Some(&home()));
        assert_eq!(cfg.max_suggestions, 9);
    }

    #[test]
    fn quoted_values_are_unquoted() {
        let text = "history_file = \"~/quoted_history\"\n";
        let cfg = Config::parse(text, Some(&home()));
        assert_eq!(cfg.history_file, home().join("quoted_history"));
    }

    #[test]
    fn absolute_history_file_path_is_kept_as_is() {
        let text = "history_file = /var/log/some_history\n";
        let cfg = Config::parse(text, Some(&home()));
        assert_eq!(cfg.history_file, PathBuf::from("/var/log/some_history"));
    }

    #[test]
    fn tilde_expansion_without_home_falls_back_to_literal_path() {
        let cfg = Config::parse("", None);
        assert_eq!(cfg.history_file, PathBuf::from(DEFAULT_HISTORY_FILE));
    }

    #[test]
    fn unrecognized_keys_are_ignored() {
        let text = "not_a_real_key = whatever\nmax_suggestions = 2\n";
        let cfg = Config::parse(text, Some(&home()));
        assert_eq!(cfg.max_suggestions, 2);
    }

    #[test]
    fn invalid_numeric_value_falls_back_to_default() {
        let text = "max_suggestions = not_a_number\n";
        let cfg = Config::parse(text, Some(&home()));
        assert_eq!(cfg.max_suggestions, DEFAULT_MAX_SUGGESTIONS);
    }
}
