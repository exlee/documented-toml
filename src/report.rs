//! What the merge noticed on the way through.

use std::collections::BTreeMap;

use crate::path::DottedPath;

/// Everything the merge has to say about the user's document.
#[derive(Debug, Clone, PartialEq, Eq, Default)]
pub struct Report {
    pub(crate) diagnostics: Vec<Diagnostic>,
}

/// One observation about one key, positioned in the user's source.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Diagnostic {
    /// What was observed.
    pub kind: DiagnosticKind,
    /// Whether an application should refuse to start.
    pub severity: Severity,
    /// The key this concerns.
    pub path: DottedPath,
    /// 1-based line in the user's source.
    pub line: usize,
    /// 1-based column in the user's source.
    pub column: usize,
}

/// The kinds of observation a merge produces.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum DiagnosticKind {
    /// The defaults do not declare this key. The value is kept, never deleted.
    UnknownKey,
    /// The user's value has a different TOML type from the default's. The value
    /// is kept exactly as written.
    TypeMismatch {
        /// The type the defaults declare.
        expected: TomlType,
        /// The type the user wrote.
        found: TomlType,
    },
    /// A rename rule moved this value.
    Migrated {
        /// Where the value was before the rule ran.
        from: DottedPath,
    },
}

/// Whether a diagnostic is worth stopping for.
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub enum Severity {
    /// Reported, but the merged document is usable.
    Warning,
    /// The document holds something the defaults cannot account for.
    Error,
}

/// The TOML types a value can have, as the merge compares them.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum TomlType {
    /// A string.
    String,
    /// An integer.
    Integer,
    /// A float.
    Float,
    /// A boolean.
    Boolean,
    /// An offset or local date, time, or date-time.
    Datetime,
    /// An array.
    Array,
    /// An inline table.
    InlineTable,
    /// A standalone table.
    Table,
    /// An array of tables.
    ArrayOfTables,
}

/// Byte spans harvested from the user document before it is converted to
/// `toml_edit::DocumentMut`, which discards them.
///
/// Positions cannot be recovered afterwards, so this is built first and carried
/// through the merge.
#[derive(Debug, Clone, PartialEq, Eq, Default)]
pub struct SpanIndex {
    pub(crate) spans: BTreeMap<DottedPath, Span>,
}

/// A byte range in the user's source.
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub struct Span {
    /// First byte, inclusive.
    pub start: usize,
    /// Last byte, exclusive.
    pub end: usize,
}

/// A 1-based line and column in the user's source.
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub struct Position {
    /// 1-based line.
    pub line: usize,
    /// 1-based column.
    pub column: usize,
}
