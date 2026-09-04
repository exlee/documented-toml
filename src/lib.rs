//! Format-preserving merge of a user's TOML configuration with the defaults an
//! application ships.
//!
//! See `doc/design.md` for the merge rules and `corpus/` for the merge output
//! they produce, which is where the behaviour is specified.
//!
//! ```
//! let merged = toml_merge::merge("##: How many.\ncount = 1\n", "count = 7\n")?;
//! assert_eq!(merged.to_toml_string(), "##: How many.\n#: count = 1\ncount = 7\n");
//! # Ok::<(), toml_merge::Error>(())
//! ```

pub mod anchor;
pub mod decor;
pub mod error;
pub mod merge;
pub mod options;
pub mod path;
pub mod report;
pub mod source;

pub use anchor::Anchor;
pub use decor::{
    DEFAULT_PROSE_MARKER, DEFAULT_SAMPLE_MARKER, DefaultEcho, DocBlock, Marker, Prefix, PrefixLine,
    Sample,
};
pub use error::Error;
pub use merge::{MergeEngine, Merged};
pub use options::{MergeOptions, Migration};
pub use path::DottedPath;
pub use report::{
    Diagnostic, DiagnosticKind, Position, Report, Severity, Span, SpanIndex, TomlType,
};
pub use source::SourceDocument;

/// Merges a user document against the defaults with the default marker and no
/// rename rules. See [`MergeOptions`] for the configurable form.
pub fn merge(default_src: &str, user_src: &str) -> Result<Merged, Error> {
    MergeOptions::new().merge(default_src, user_src)
}
