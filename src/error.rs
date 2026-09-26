//! Failures that stop a merge before it starts.

use toml_edit::TomlError;

/// A document that could not be parsed. Which one is part of the error, because
/// a broken default document is the application's fault and a broken user
/// document is the person's.
#[derive(Debug)]
pub enum Error {
    /// The shipped default document failed to parse.
    DefaultParse {
        /// The underlying parse failure.
        source: TomlError,
    },
    /// The user's document failed to parse.
    UserParse {
        /// The underlying parse failure.
        source: TomlError,
    },
    /// The defaults parsed and hold text, but declare no keys and document
    /// none either.
    ///
    /// A defaults document made only of the maintainers' own notes has nothing
    /// to merge into and nothing to say. That is an application bug, so it
    /// fails here instead of becoming a state the merge has to carry. A
    /// document of marker lines alone is not this: an option anchored on a key
    /// the application ships no value for is documented without declaring
    /// anything, and a defaults file of nothing but such blocks is a whole
    /// configuration. A zero-byte defaults document is a separate case and is
    /// allowed, see [`SourceDocument::Empty`](crate::SourceDocument::Empty).
    DefaultsDeclareNoKeys,
    /// A rename rule was given a path that is not a TOML key path.
    MigrationPath {
        /// The underlying parse failure.
        source: TomlError,
    },
    /// [`Merged::set`](crate::Merged::set) was given a path that is not a
    /// TOML key path.
    SetPath {
        /// The underlying parse failure.
        source: TomlError,
    },
    /// [`Merged::set`](crate::Merged::set) was given a path where a table or
    /// an array of tables already sits. Writing a value there would throw
    /// away everything under it.
    NotAValue {
        /// The path as it was given.
        path: String,
    },
}

impl std::fmt::Display for Error {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Self::DefaultParse { source } => {
                write!(f, "the default document does not parse: {source}")
            }
            Self::UserParse { source } => write!(f, "the user document does not parse: {source}"),
            Self::DefaultsDeclareNoKeys => f.write_str("the default document declares no keys"),
            Self::MigrationPath { source } => {
                write!(f, "a rename rule has an unreadable path: {source}")
            }
            Self::SetPath { source } => write!(f, "unreadable path: {source}"),
            Self::NotAValue { path } => {
                write!(f, "{path} holds a table, so no value can be set there")
            }
        }
    }
}

impl std::error::Error for Error {
    fn source(&self) -> Option<&(dyn std::error::Error + 'static)> {
        match self {
            Self::DefaultParse { source }
            | Self::UserParse { source }
            | Self::MigrationPath { source }
            | Self::SetPath { source } => Some(source),
            Self::DefaultsDeclareNoKeys | Self::NotAValue { .. } => None,
        }
    }
}
