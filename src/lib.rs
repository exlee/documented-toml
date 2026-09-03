//! Format-preserving merge of a user's TOML configuration with the defaults an
//! application ships.
//!
//! See `doc/design.md` for the merge rules and `doc/model/model.yml` for the
//! structural model these types are drawn from.
//!
//! This is the type skeleton. No merge behaviour is implemented yet.

pub mod decor;
pub mod error;
pub mod merge;
pub mod options;
pub mod path;
pub mod report;

pub use decor::{DefaultEcho, DocBlock, Marker, Prefix, PrefixLine};
pub use error::Error;
pub use merge::{MergeEngine, Merged};
pub use options::{MergeOptions, Migration};
pub use path::DottedPath;
pub use report::{
    Diagnostic, DiagnosticKind, Position, Report, Severity, Span, SpanIndex, TomlType,
};
