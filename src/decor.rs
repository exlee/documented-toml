//! Classification and reassembly of the text above a key.
//!
//! A `toml_edit::Decor` prefix is one raw string covering everything between
//! the previous item and this key: blank lines, indentation and comment lines
//! together. These types split that string and build the replacement.

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

/// The raw decor text between the previous item and this key.
#[derive(Debug, Clone, PartialEq, Eq, Default)]
pub struct Prefix {
    pub(crate) raw: String,
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
