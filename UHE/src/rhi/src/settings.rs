//! A settings file, and the part of it that chooses a backend.
//!
//! A `key = value` file with no sections, because the only key today is
//! `backend`. No path searching or environment reading: [`Settings::parse`] takes
//! a string, so the library is testable without a filesystem.
//!
//! ```text
//! # unhuman.settings
//! backend = vulkan
//! ```
//!
//! Blank lines and `#` comments are ignored. Keys are case-insensitive; unknown
//! keys are ignored rather than refused, see [`Settings::parse`].

use std::collections::BTreeMap;

/// The file name looked for in the working directory.
pub const FILE_NAME: &str = "unhuman.settings";

/// The environment variable naming a settings file, for when the working
/// directory is not where the settings live.
///
/// Deliberately a *path* rather than a value: a `UHE_BACKEND`-style variable
/// holding the answer and a file both saying "settings" invites a precedence
/// question. This one only says where the file is.
pub const PATH_VARIABLE: &str = "UHE_SETTINGS";

/// Parsed settings.
///
/// A `BTreeMap` rather than a `HashMap` so iteration -- and therefore any
/// future "dump the settings" output -- is in a stable order. There is one key
/// today and determinism costs nothing to keep.
#[derive(Clone, Debug, Default, PartialEq, Eq)]
pub struct Settings {
    entries: BTreeMap<String, String>,
}

impl Settings {
    pub fn parse(text: &str) -> Self {
        let mut entries = BTreeMap::new();
        for line in text.lines() {
            let line = line.split('#').next().unwrap_or_default().trim();
            if line.is_empty() {
                continue;
            }
            // A line with no `=` is skipped rather than refused. It is almost
            // always a comment style this parser does not know, and a settings
            // file that refuses to load because of a stray `;` is a file people
            // stop editing.
            let Some((key, value)) = line.split_once('=') else {
                continue;
            };
            let key = key.trim();
            if key.is_empty() {
                continue;
            }
            // Last one wins, which is what every other config format does and
            // what makes appending a line to the bottom an override.
            entries.insert(
                key.to_ascii_lowercase(),
                value.trim().trim_matches('"').trim_matches('\'').to_owned(),
            );
        }
        Self { entries }
    }

    /// One setting, by name, case-insensitively.
    pub fn get(&self, key: &str) -> Option<&str> {
        self.entries
            .get(&key.to_ascii_lowercase())
            .map(String::as_str)
    }

    /// The backend this file asks for, if it asks at all.
    ///
    /// `None` for a file with no `backend` key -- which is the normal case, and
    /// is why this is an `Option` and not an empty string that then fails to
    /// parse.
    pub fn backend(&self) -> Option<&str> {
        self.get("backend").filter(|value| !value.is_empty())
    }

    /// Every key, for a log line that says what a settings file actually did.
    pub fn keys(&self) -> impl Iterator<Item = &str> {
        self.entries.keys().map(String::as_str)
    }

    pub fn is_empty(&self) -> bool {
        self.entries.is_empty()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn the_obvious_file_parses() {
        let settings = Settings::parse("# unhuman.settings\nbackend = vulkan\n");
        assert_eq!(settings.backend(), Some("vulkan"));
    }

    #[test]
    fn a_file_with_no_backend_key_has_none() {
        let settings = Settings::parse("window.width = 1280\n");
        assert_eq!(settings.backend(), None);
        assert!(!settings.is_empty(), "the width is still a setting");
    }

    #[test]
    fn an_empty_file_is_empty_and_is_not_an_error() {
        assert!(Settings::parse("").is_empty());
        assert!(Settings::parse("\n\n   \n# just a comment\n").is_empty());
    }

    /// An empty value is "nobody said", not "the empty backend".
    ///
    /// The distinction matters because `--backend ""` and a settings line
    /// `backend =` look identical here, and one of them should fall through to
    /// the next source while the other should not.
    #[test]
    fn an_empty_value_is_not_a_backend() {
        assert_eq!(Settings::parse("backend =\n").backend(), None);
        assert_eq!(Settings::parse("backend =   \n").backend(), None);
    }

    #[test]
    fn keys_and_values_are_trimmed_and_quotes_stripped() {
        let settings = Settings::parse("  backend  =  \"vulkan\"  \n");
        assert_eq!(settings.backend(), Some("vulkan"));
    }

    /// Key case does not matter; value case does, because `VULKAN` and `vulkan`
    /// both parse but `Vulkan` might not.
    #[test]
    fn keys_are_case_insensitive_and_values_are_not() {
        assert_eq!(
            Settings::parse("BACKEND = vulkan\n").backend(),
            Some("vulkan")
        );
    }

    /// The last one wins, so appending a line at the bottom overrides.
    #[test]
    fn the_last_value_wins() {
        let settings = Settings::parse("backend = wgpu\nbackend = vulkan\n");
        assert_eq!(settings.backend(), Some("vulkan"));
    }

    /// A trailing `#` comment is stripped; a `#` inside a value is not, because
    /// there is no quoting rule that would make that unambiguous.
    #[test]
    fn a_trailing_comment_is_stripped() {
        assert_eq!(
            Settings::parse("backend = vulkan # the good one\n").backend(),
            Some("vulkan")
        );
    }

    /// A line with no `=` is ignored rather than refused.
    ///
    /// It is almost always a comment style this parser does not know, and a
    /// settings file that refuses to load because of a stray `;` is a file
    /// people stop editing.
    #[test]
    fn a_line_that_is_not_a_pair_is_ignored() {
        let settings = Settings::parse("; ini style comment\nbackend = vulkan\n");
        assert_eq!(settings.backend(), Some("vulkan"));
    }

    #[test]
    fn keys_come_out_sorted_and_stable() {
        let settings = Settings::parse("zebra = 1\nalpha = 2\nmiddle = 3\n");
        assert_eq!(
            settings.keys().collect::<Vec<_>>(),
            ["alpha", "middle", "zebra"]
        );
    }

    #[test]
    fn a_value_containing_an_equals_sign_is_kept_whole() {
        let settings = Settings::parse("args = --a=b\n");
        assert_eq!(settings.get("args"), Some("--a=b"));
    }
}
