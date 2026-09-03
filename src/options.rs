//! What the caller configures before a merge.

use crate::decor::Marker;
use crate::path::DottedPath;

/// Builder holding the marker and the rename rules.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct MergeOptions {
    pub(crate) marker: Marker,
    pub(crate) migrations: Vec<Migration>,
}

/// One rename rule, applied before the merge walk.
///
/// It fires when `from` is present in the user document and `to` is absent.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Migration {
    pub(crate) from: DottedPath,
    pub(crate) to: DottedPath,
}
