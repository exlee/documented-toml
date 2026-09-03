//! Key paths.

use std::fmt;

use nonempty::NonEmpty;
use toml_edit::{Key, TomlError};

/// A key path such as `server.timeout`.
///
/// Segments are stored unescaped, as the key text means it, so a quoted TOML
/// key holding a dot is one segment and not two. A path always names something,
/// so it always has a first segment.
#[derive(Debug, Clone, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub struct DottedPath {
    pub(crate) segments: NonEmpty<String>,
}

impl DottedPath {
    /// A path of one segment.
    pub fn new(head: impl Into<String>) -> Self {
        Self {
            segments: NonEmpty::new(head.into()),
        }
    }

    /// Reads a dotted key as TOML itself would, so `a."b.c"` is two segments
    /// and the quotes are gone from the second.
    pub fn parse(dotted: &str) -> Result<Self, TomlError> {
        let keys = Key::parse(dotted)?;
        let mut segments = keys.iter().map(|k| k.get().to_owned());
        let head = segments.next().expect("Key::parse yields at least one key");
        let mut path = Self::new(head);
        for segment in segments {
            path.segments.push(segment);
        }
        Ok(path)
    }

    /// This path with one more segment on the end.
    pub fn child(&self, segment: impl Into<String>) -> Self {
        let mut child = self.clone();
        child.segments.push(segment.into());
        child
    }

    /// The segments, unescaped, from the root down.
    pub fn segments(&self) -> impl Iterator<Item = &str> {
        self.segments.iter().map(String::as_str)
    }

    /// The last segment: the key this path names.
    pub fn leaf(&self) -> &str {
        self.segments.last()
    }
}

impl fmt::Display for DottedPath {
    /// Renders the path as TOML would write it, quoting the segments that need
    /// it, so the output can be read back by [`DottedPath::parse`].
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        for (i, segment) in self.segments.iter().enumerate() {
            if i > 0 {
                f.write_str(".")?;
            }
            f.write_str(&Key::new(segment).display_repr())?;
        }
        Ok(())
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn a_path_reads_and_writes_back_the_same() {
        for text in ["a", "server.timeout", "a.\"b.c\"", "\"has space\".x"] {
            let path = DottedPath::parse(text).unwrap();
            assert_eq!(path.to_string(), text);
        }
    }

    #[test]
    fn a_quoted_segment_holding_a_dot_is_one_segment() {
        let path = DottedPath::parse("a.\"b.c\"").unwrap();
        assert_eq!(path.segments().collect::<Vec<_>>(), ["a", "b.c"]);
        assert_eq!(path.leaf(), "b.c");
    }

    #[test]
    fn segments_are_stored_unescaped() {
        let path = DottedPath::parse("\"needs quoting\"").unwrap();
        assert_eq!(path.leaf(), "needs quoting");
    }

    #[test]
    fn a_child_extends_the_path() {
        let path = DottedPath::new("a").child("b");
        assert_eq!(path.to_string(), "a.b");
    }

    #[test]
    fn a_path_that_is_not_a_key_path_does_not_parse() {
        assert!(DottedPath::parse("a..b").is_err());
        assert!(DottedPath::parse("").is_err());
    }
}
