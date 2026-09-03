//! Classification and reassembly of the text above a key.
//!
//! A `toml_edit::Decor` prefix is one raw string covering everything between
//! the previous item and this key: blank lines, indentation and comment lines
//! together. These types split that string and build the replacement.

use toml_edit::Decor;

/// The comment prefix owned by the tool, `#:` by default.
///
/// Lines carrying it are rewritten from the defaults on every merge. Lines
/// starting with a plain `#` belong to the person and are never touched.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Marker {
    pub(crate) value: String,
}

/// The default marker, used when the caller sets none.
pub const DEFAULT_MARKER: &str = "#:";

impl Marker {
    /// A marker from its literal text, such as `#:`.
    pub fn new(value: impl Into<String>) -> Self {
        Self {
            value: value.into(),
        }
    }

    /// The marker text.
    pub fn as_str(&self) -> &str {
        &self.value
    }

    /// Whether a line belongs to the tool.
    ///
    /// It does when its first non-whitespace characters are exactly the marker
    /// and its run of `#` is no longer than the marker's. With the default
    /// marker `#:` is a marker line; `##:` and `#` are not.
    pub fn owns(&self, line: &str) -> bool {
        let text = line.trim_start();
        text.starts_with(&self.value) && hashes(text) == hashes(&self.value)
    }

    /// The line recording a shipped default, marker included.
    fn echo_line(&self, echo: &DefaultEcho) -> String {
        format!("{} {} = {}", self.value, echo.key, echo.value)
    }
}

impl Default for Marker {
    fn default() -> Self {
        Self::new(DEFAULT_MARKER)
    }
}

fn hashes(text: &str) -> usize {
    text.chars().take_while(|c| *c == '#').count()
}

/// The raw decor text between the previous item and this key.
#[derive(Debug, Clone, PartialEq, Eq, Default)]
pub struct Prefix {
    pub(crate) raw: String,
}

impl Prefix {
    /// The prefix of a decor, empty when it has none.
    pub fn of(decor: &Decor) -> Self {
        let raw = decor
            .prefix()
            .and_then(|raw| raw.as_str())
            .unwrap_or_default()
            .to_owned();
        Self { raw }
    }

    /// The complete lines of the prefix, classified.
    ///
    /// The text after the last newline is indentation for the key itself, not a
    /// line of its own, and comes back from [`Prefix::indent`].
    pub fn lines(&self, marker: &Marker) -> Vec<PrefixLine> {
        let mut parts: Vec<&str> = self.raw.split('\n').collect();
        parts.pop();
        parts
            .into_iter()
            .map(|text| {
                let text = text.to_owned();
                if text.trim().is_empty() {
                    PrefixLine::Blank { text }
                } else if marker.owns(&text) {
                    PrefixLine::Marker { text }
                } else {
                    PrefixLine::User { text }
                }
            })
            .collect()
    }

    /// The whitespace between the last newline and the key.
    pub fn indent(&self) -> &str {
        match self.raw.rfind('\n') {
            Some(at) => &self.raw[at + 1..],
            None => &self.raw,
        }
    }
}

/// One classified line of a [`Prefix`].
///
/// The text is kept verbatim, including leading whitespace, so a line the merge
/// does not own can be written back exactly as it was found.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum PrefixLine {
    /// Owned by the tool. Rewritten on every merge.
    Marker {
        /// The line as it appeared, marker included.
        text: String,
    },
    /// Owned by the person. Never rewritten, reflowed or reordered.
    User {
        /// The line as it appeared.
        text: String,
    },
    /// Empty or whitespace only.
    Blank {
        /// The line as it appeared.
        text: String,
    },
}

/// One key's rebuilt prefix.
///
/// Rendered in this order: the leading blank lines the user had, the
/// documentation lines taken from the current defaults, the echo of the shipped
/// default when the value was overridden, then the user's own comment lines.
/// The person's notes end up closest to the key they annotate.
#[derive(Debug, Clone, PartialEq, Eq, Default)]
pub struct DocBlock {
    /// Blank lines above the block, as [`PrefixLine::Blank`] text.
    pub(crate) leading_blanks: Vec<String>,
    /// Documentation from the defaults, as [`PrefixLine::Marker`] text.
    pub(crate) doc_lines: Vec<String>,
    /// The shipped default, present only when the user's value differs.
    pub(crate) echo: Option<DefaultEcho>,
    /// The person's own comments, as [`PrefixLine::User`] text.
    pub(crate) user_lines: Vec<String>,
    /// Whitespace before the key itself, taken from whoever supplied the key.
    pub(crate) indent: String,
}

impl DocBlock {
    /// Takes the leading blank lines, the person's comments and the
    /// indentation from a prefix. The marker lines in it are dropped: they
    /// belong to the tool and are written again from the defaults.
    pub(crate) fn keep_user_text(&mut self, prefix: &Prefix, marker: &Marker) {
        let lines = prefix.lines(marker);
        let mut seen_content = false;
        for line in lines {
            match line {
                PrefixLine::Blank { text } if !seen_content => self.leading_blanks.push(text),
                PrefixLine::Blank { .. } => {}
                PrefixLine::Marker { .. } => seen_content = true,
                PrefixLine::User { text } => {
                    seen_content = true;
                    self.user_lines.push(text);
                }
            }
        }
        self.indent = prefix.indent().to_owned();
    }

    /// Takes the documentation lines from a prefix in the default document.
    /// Plain comments there are notes for whoever maintains the defaults and
    /// never reach a user's file.
    pub(crate) fn take_docs(&mut self, prefix: &Prefix, marker: &Marker) {
        self.doc_lines = prefix
            .lines(marker)
            .into_iter()
            .filter_map(|line| match line {
                PrefixLine::Marker { text } => Some(text),
                _ => None,
            })
            .collect();
    }

    /// The prefix text to write back.
    pub(crate) fn render(&self, marker: &Marker) -> String {
        let mut out = String::new();
        for line in &self.leading_blanks {
            out.push_str(line);
            out.push('\n');
        }
        for line in &self.doc_lines {
            out.push_str(line);
            out.push('\n');
        }
        if let Some(echo) = &self.echo {
            out.push_str(&marker.echo_line(echo));
            out.push('\n');
        }
        for line in &self.user_lines {
            out.push_str(line);
            out.push('\n');
        }
        out.push_str(&self.indent);
        out
    }
}

/// The marker line recording the shipped default.
///
/// Written only when the user's value differs from it. A key whose value equals
/// the default, or one the merge inserted, gets no echo: the live value already
/// is the default.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct DefaultEcho {
    pub(crate) key: String,
    pub(crate) value: String,
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn the_marker_owns_its_own_lines_only() {
        let marker = Marker::default();
        assert!(marker.owns("#: documentation"));
        assert!(marker.owns("   #: indented"));
        assert!(marker.owns("#:"));
        assert!(!marker.owns("# a plain comment"));
        assert!(!marker.owns("##: one hash too many"));
        assert!(!marker.owns("value = 1"));
    }

    #[test]
    fn a_different_marker_moves_which_lines_are_owned() {
        let marker = Marker::new("#!");
        assert!(marker.owns("#! mine"));
        assert!(!marker.owns("#: not mine"));
    }

    #[test]
    fn a_prefix_splits_into_whole_lines_and_the_indentation_after_them() {
        let prefix = Prefix {
            raw: "\n#: doc\n# note\n  ".to_owned(),
        };
        let marker = Marker::default();
        assert_eq!(
            prefix.lines(&marker),
            vec![
                PrefixLine::Blank {
                    text: String::new()
                },
                PrefixLine::Marker {
                    text: "#: doc".to_owned()
                },
                PrefixLine::User {
                    text: "# note".to_owned()
                },
            ]
        );
        assert_eq!(prefix.indent(), "  ");
    }

    #[test]
    fn a_prefix_with_no_newline_is_all_indentation() {
        let prefix = Prefix {
            raw: "  ".to_owned(),
        };
        assert!(prefix.lines(&Marker::default()).is_empty());
        assert_eq!(prefix.indent(), "  ");
    }

    #[test]
    fn blanks_below_the_first_comment_are_not_kept() {
        let marker = Marker::default();
        let mut block = DocBlock::default();
        block.keep_user_text(
            &Prefix {
                raw: "\n\n# note\n\n".to_owned(),
            },
            &marker,
        );
        assert_eq!(block.leading_blanks.len(), 2);
        assert_eq!(block.user_lines, ["# note"]);
    }

    #[test]
    fn a_block_renders_the_person_closest_to_their_key() {
        let marker = Marker::default();
        let block = DocBlock {
            leading_blanks: vec![String::new()],
            doc_lines: vec!["#: what it does".to_owned()],
            echo: Some(DefaultEcho {
                key: "count".to_owned(),
                value: "1".to_owned(),
            }),
            user_lines: vec!["# why I changed it".to_owned()],
            indent: String::new(),
        };
        assert_eq!(
            block.render(&marker),
            "\n#: what it does\n#: count = 1\n# why I changed it\n"
        );
    }

    #[test]
    fn documentation_is_taken_from_the_marker_lines_alone() {
        let marker = Marker::default();
        let mut block = DocBlock::default();
        block.take_docs(
            &Prefix {
                raw: "# maintainer note\n#: reaches the user\n".to_owned(),
            },
            &marker,
        );
        assert_eq!(block.doc_lines, ["#: reaches the user"]);
    }
}
