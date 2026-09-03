//! Key paths.

use nonempty::NonEmpty;

/// A key path such as `server.timeout`.
///
/// Segments are stored unescaped, as the key text means it, so a quoted TOML
/// key holding a dot is one segment and not two. A path always names something,
/// so it always has a first segment.
#[derive(Debug, Clone, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub struct DottedPath {
    pub(crate) segments: NonEmpty<String>,
}
