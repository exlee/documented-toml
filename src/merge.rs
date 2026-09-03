//! The merge itself: what runs it, and what comes out.

use toml_edit::DocumentMut;

use crate::options::MergeOptions;
use crate::report::{Report, SpanIndex};
use crate::source::SourceDocument;

/// The merged document and its report.
///
/// This is also the effective configuration. Every default key is materialised
/// with a live value, so the file written back and the configuration a caller
/// deserializes cannot drift apart.
// The accessors that read these arrive with the merge engine.
#[allow(dead_code)]
#[derive(Debug, Clone)]
pub struct Merged {
    /// Everything the merge noticed.
    pub report: Report,
    pub(crate) document: DocumentMut,
}

/// Walks the default document as the spine and transplants user values.
///
/// The defaults give the shape and the order; the user gives the values.
#[allow(dead_code)]
#[derive(Debug, Clone)]
pub struct MergeEngine {
    /// The shipped document, which gives the shape and the order.
    pub(crate) defaults: SourceDocument,
    /// The person's document, which gives the values.
    pub(crate) user: SourceDocument,
    pub(crate) options: MergeOptions,
    pub(crate) spans: SpanIndex,
    pub(crate) report: Report,
}
