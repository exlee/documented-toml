//! What the merge noticed on the way through.

use std::collections::BTreeMap;

use toml_edit::{Item, Table, Value};

use crate::path::DottedPath;

/// Everything the merge has to say about the user's document.
#[derive(Debug, Clone, PartialEq, Eq, Default)]
pub struct Report {
    pub(crate) diagnostics: Vec<Diagnostic>,
}

impl Report {
    /// Everything the merge noticed, in the order it was noticed.
    pub fn diagnostics(&self) -> &[Diagnostic] {
        &self.diagnostics
    }

    /// Whether anything in the report is an [`Severity::Error`].
    pub fn has_errors(&self) -> bool {
        self.diagnostics
            .iter()
            .any(|d| d.severity == Severity::Error)
    }

    pub(crate) fn push(&mut self, diagnostic: Diagnostic) {
        self.diagnostics.push(diagnostic);
    }
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

impl Diagnostic {
    /// The severity every diagnostic of a given kind carries.
    pub(crate) fn severity_of(kind: &DiagnosticKind) -> Severity {
        match kind {
            DiagnosticKind::UnknownKey | DiagnosticKind::Migrated { .. } => Severity::Warning,
            DiagnosticKind::TypeMismatch { .. } => Severity::Error,
        }
    }

    pub(crate) fn new(kind: DiagnosticKind, path: DottedPath, at: Position) -> Self {
        Self {
            severity: Self::severity_of(&kind),
            kind,
            path,
            line: at.line,
            column: at.column,
        }
    }
}

/// The kinds of observation a merge produces.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum DiagnosticKind {
    /// The defaults do not declare this key. The value is kept, never deleted.
    UnknownKey,
    /// The user's value has an incompatible TOML type. Integer values are
    /// accepted for float defaults. The value is kept exactly as written.
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

impl std::fmt::Display for DiagnosticKind {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Self::UnknownKey => f.write_str("no such option in the defaults"),
            Self::TypeMismatch { expected, found } => {
                write!(f, "expected {expected}, found {found}")
            }
            Self::Migrated { from } => write!(f, "moved from {from}"),
        }
    }
}

/// Whether a diagnostic is worth stopping for.
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub enum Severity {
    /// Reported, but the merged document is usable.
    Warning,
    /// The document holds something the defaults cannot account for.
    Error,
}

impl std::fmt::Display for Severity {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.write_str(match self {
            Self::Warning => "warning",
            Self::Error => "error",
        })
    }
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

impl SpanIndex {
    /// Harvests the key spans of a document parsed with spans still on it.
    pub(crate) fn build(root: &Table) -> Self {
        let mut index = Self::default();
        index.walk(root, None);
        index
    }

    fn walk(&mut self, table: &Table, path: Option<&DottedPath>) {
        for (name, item) in table.iter() {
            let child = match path {
                Some(path) => path.child(name),
                None => DottedPath::new(name),
            };
            if let Some(key) = table.key(name)
                && let Some(span) = key.span()
            {
                self.spans.insert(
                    child.clone(),
                    Span {
                        start: span.start,
                        end: span.end,
                    },
                );
            }
            // Entries of an array of tables are never walked into: the user's
            // array replaces the default one whole, so nothing inside it is
            // ever compared against a default and nothing there is reported.
            if let Item::Table(sub) = item {
                self.walk(sub, Some(&child));
            }
        }
    }

    /// Where a key sits in the user's source, or the start of the document when
    /// the key is not one the user wrote.
    pub(crate) fn position(&self, src: &str, path: &DottedPath) -> Position {
        match self.spans.get(path) {
            Some(span) => Position::of(src, span.start),
            None => Position { line: 1, column: 1 },
        }
    }
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

impl Position {
    /// The line and column of a byte offset. The column counts characters, so a
    /// multi-byte character before the key counts once.
    pub(crate) fn of(src: &str, offset: usize) -> Self {
        let offset = offset.min(src.len());
        let before = &src[..offset];
        let line = before.matches('\n').count() + 1;
        let column = match before.rfind('\n') {
            Some(at) => before[at + 1..].chars().count() + 1,
            None => before.chars().count() + 1,
        };
        Self { line, column }
    }
}

impl TomlType {
    /// The type an item has, as the merge compares types.
    pub(crate) fn of(item: &Item) -> Option<Self> {
        match item {
            Item::Value(value) => Some(Self::of_value(value)),
            Item::Table(_) => Some(Self::Table),
            Item::ArrayOfTables(_) => Some(Self::ArrayOfTables),
            Item::None => None,
        }
    }

    fn of_value(value: &Value) -> Self {
        match value {
            Value::String(_) => Self::String,
            Value::Integer(_) => Self::Integer,
            Value::Float(_) => Self::Float,
            Value::Boolean(_) => Self::Boolean,
            Value::Datetime(_) => Self::Datetime,
            Value::Array(_) => Self::Array,
            Value::InlineTable(_) => Self::InlineTable,
        }
    }

    /// The name TOML gives the type.
    pub fn name(self) -> &'static str {
        match self {
            Self::String => "string",
            Self::Integer => "integer",
            Self::Float => "float",
            Self::Boolean => "boolean",
            Self::Datetime => "datetime",
            Self::Array => "array",
            Self::InlineTable => "inline table",
            Self::Table => "table",
            Self::ArrayOfTables => "array of tables",
        }
    }
}

impl std::fmt::Display for TomlType {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.write_str(self.name())
    }
}
