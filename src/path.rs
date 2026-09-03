//! Key paths.

/// A key path such as `server.timeout`.
///
/// Segments are stored unescaped, as the key text means it, so a quoted TOML
/// key holding a dot is one segment and not two.
#[derive(Debug, Clone, PartialEq, Eq, PartialOrd, Ord, Hash, Default)]
pub struct DottedPath {
    pub(crate) segments: Vec<String>,
}
