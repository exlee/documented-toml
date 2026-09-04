//! Format-preserving merge of a person's TOML configuration with the documented
//! defaults an application ships.
//!
//! The crate documentation is the README, so its examples are compiled and run
//! with the rest of the tests. `doc/design.md` states the merge rules and
//! `corpus/` holds the output they produce, which is where the behaviour is
//! specified.
#![doc = include_str!("../README.md")]

pub mod decor;
pub mod error;
pub mod merge;
pub mod options;
pub mod path;
pub mod report;
pub mod source;
pub mod template;

pub use decor::{
    DEFAULT_PROSE_MARKER, DEFAULT_SAMPLE_MARKER, DefaultEcho, DocBlock, Marker, Prefix, PrefixLine,
    Sample,
};
pub use error::Error;
pub use merge::{MergeEngine, Merged, Newline};
pub use options::{MergeOptions, Migration};
pub use path::DottedPath;
pub use report::{
    Diagnostic, DiagnosticKind, Position, Report, Severity, Span, SpanIndex, TomlType,
};
pub use source::SourceDocument;
pub use template::Template;

/// Merges a user document against the defaults with the default marker and no
/// rename rules. See [`MergeOptions`] for the configurable form.
pub fn merge(default_src: &str, user_src: &str) -> Result<Merged, Error> {
    MergeOptions::new().merge(default_src, user_src)
}
