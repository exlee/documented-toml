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
    /// The defaults parsed and hold text, but declare no keys.
    ///
    /// A defaults document made only of comments has nothing to merge into and
    /// nothing to document. That is an application bug, so it fails here
    /// instead of becoming a state the merge has to carry. A zero-byte defaults
    /// document is a separate case and is allowed, see
    /// [`SourceDocument::Empty`](crate::SourceDocument::Empty).
    DefaultsDeclareNoKeys,
    /// A rename rule was given a path that is not a TOML key path.
    MigrationPath {
        /// The underlying parse failure.
        source: TomlError,
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
        }
    }
}

impl std::error::Error for Error {
    fn source(&self) -> Option<&(dyn std::error::Error + 'static)> {
        match self {
            Self::DefaultParse { source }
            | Self::UserParse { source }
            | Self::MigrationPath { source } => Some(source),
            Self::DefaultsDeclareNoKeys => None,
        }
    }
}
