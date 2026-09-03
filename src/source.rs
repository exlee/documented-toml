//! Input documents, before the merge walks them.

use toml_edit::Table;

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
