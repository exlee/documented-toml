//! Input documents, before the merge walks them.

use toml_edit::{Document, DocumentMut, Table, TomlError};

use crate::report::SpanIndex;

/// One of the two documents a merge is given, after parsing.
///
/// Zero bytes and "parsed, but declares no keys" are different states, and only
/// byte length decides which. A file holding `# notes to self` is not empty: it
/// has text that has to survive the merge, and default keys are written in
/// beneath it. Only a file of no length has nothing to preserve.
#[derive(Debug, Clone)]
pub enum SourceDocument {
    /// Zero bytes. Nothing to parse, nothing to keep.
    Empty,
    /// Text that parsed. The root table may hold no keys, which is a document
    /// with content all the same.
    Content {
        /// The root table. Every key, table and array of tables in the document
        /// nests inside it.
        root: Table,
        /// Whitespace and comments after the last element, which belong to no
        /// key. A comment-only document is all trailing and no keys.
        trailing: String,
    },
}

impl SourceDocument {
    /// Parses a document, keeping nothing but the root table and the trailing
    /// text. Spans are discarded; use [`SourceDocument::parse_with_spans`] for
    /// the document diagnostics point into.
    pub fn parse(src: &str) -> Result<Self, TomlError> {
        Ok(Self::parse_with_spans(src)?.0)
    }

    /// Parses a document and harvests its key spans first.
    ///
    /// `Document::into_mut` resolves spans into owned strings and the
    /// `span()` accessors then return `None`, so positions have to be taken
    /// before the conversion. There is no way back.
    pub fn parse_with_spans(src: &str) -> Result<(Self, SpanIndex), TomlError> {
        if src.is_empty() {
            return Ok((Self::Empty, SpanIndex::default()));
        }
        let parsed = Document::parse(src)?;
        let spans = SpanIndex::build(parsed.as_table());
        let document: DocumentMut = parsed.into_mut();
        let trailing = document.trailing().as_str().unwrap_or_default().to_owned();
        Ok((
            Self::Content {
                root: document.into_table(),
                trailing,
            },
            spans,
        ))
    }

    /// The root table, or `None` for a zero-byte document.
    pub fn root(&self) -> Option<&Table> {
        match self {
            Self::Empty => None,
            Self::Content { root, .. } => Some(root),
        }
    }

    /// The root table, mutable, or `None` for a zero-byte document.
    pub(crate) fn root_mut(&mut self) -> Option<&mut Table> {
        match self {
            Self::Empty => None,
            Self::Content { root, .. } => Some(root),
        }
    }

    /// The text after the last element, empty for a zero-byte document.
    pub fn trailing(&self) -> &str {
        match self {
            Self::Empty => "",
            Self::Content { trailing, .. } => trailing,
        }
    }

    /// Whether the document parsed and declares no keys. A zero-byte document
    /// is not one of these: it has no text at all.
    pub fn has_text_but_no_keys(&self) -> bool {
        match self {
            Self::Empty => false,
            Self::Content { root, .. } => root.is_empty(),
        }
    }
}
