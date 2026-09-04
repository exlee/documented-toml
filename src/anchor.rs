//! What a run of sample lines says it documents.
//!
//! A `#:` line is TOML text, so it names a key. That name is an anchor: it ties
//! a block of documentation to an option even when the defaults ship no live
//! value for it, and it says where the block belongs once the person declares
//! that option for real.

use crate::decor::{Marker, PrefixLine};
use crate::path::DottedPath;

/// The key a block of documentation is about.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Anchor {
    /// The key the sample lines declare.
    pub path: DottedPath,
    /// Whether the sample opened a `[table]` header, which names a key from the
    /// root, as against a bare assignment naming one in the table it sits in.
    pub absolute: bool,
}

impl Anchor {
    /// Reads the first sample line of a block and takes the key it names.
    ///
    /// The first line is what the block is about: `[[accounts]]` opens a
    /// template for `accounts`, `editor = "kak"` is the sample for `editor`.
    /// Only that line is read, so a block going on to show several variants of
    /// the same table, or a value spanning several lines, still anchors. A
    /// block with no sample lines is prose, and anchors nothing.
    pub fn of(lines: &[PrefixLine], marker: &Marker) -> Option<Self> {
        let first = lines
            .iter()
            .filter_map(|line| match line {
                PrefixLine::Sample { text } => Some(marker.undress(text)),
                _ => None,
            })
            .find(|body| !body.trim().is_empty())?;
        let body = first.trim();

        if let Some(inner) = strip_header(body, "[[", "]]").or_else(|| strip_header(body, "[", "]"))
        {
            return Some(Self {
                path: DottedPath::parse(inner).ok()?,
                absolute: true,
            });
        }
        let (name, _) = body.split_once('=')?;
        Some(Self {
            path: DottedPath::parse(name.trim()).ok()?,
            absolute: false,
        })
    }

    /// The path this anchor names, read from the table the block sits in.
    pub fn resolve(&self, within: Option<&DottedPath>) -> DottedPath {
        match (self.absolute, within) {
            (false, Some(table)) => {
                let mut path = table.clone();
                for segment in self.path.segments() {
                    path = path.child(segment);
                }
                path
            }
            _ => self.path.clone(),
        }
    }
}

/// The key path inside a `[table]` or `[[array]]` header, with any trailing
/// comment left off.
fn strip_header<'a>(body: &'a str, open: &str, close: &str) -> Option<&'a str> {
    let rest = body.strip_prefix(open)?;
    let end = rest.find(close)?;
    Some(&rest[..end])
}

#[cfg(test)]
mod tests {
    use super::*;

    fn lines(text: &str) -> Vec<PrefixLine> {
        let marker = Marker::default();
        text.lines().map(|line| marker.classify(line)).collect()
    }

    #[test]
    fn a_bare_assignment_names_a_key_in_the_table_it_sits_in() {
        let anchor =
            Anchor::of(&lines("##: prose\n#: editor = \"kak\""), &Marker::default()).unwrap();
        assert_eq!(anchor.path.to_string(), "editor");
        assert!(!anchor.absolute);
        assert_eq!(
            anchor
                .resolve(Some(&DottedPath::new("general")))
                .to_string(),
            "general.editor"
        );
    }

    #[test]
    fn a_table_header_names_a_key_from_the_root() {
        let anchor = Anchor::of(
            &lines("#: [[accounts]]\n#: name = \"Personal\"\n#: [accounts.ui]\n#: separators = []"),
            &Marker::default(),
        )
        .unwrap();
        assert_eq!(anchor.path.to_string(), "accounts");
        assert!(anchor.absolute);
        assert_eq!(
            anchor.resolve(Some(&DottedPath::new("flags"))).to_string(),
            "accounts"
        );
    }

    #[test]
    fn prose_alone_anchors_nothing() {
        assert!(
            Anchor::of(
                &lines("##: an explanation and nothing else"),
                &Marker::default()
            )
            .is_none()
        );
    }

    #[test]
    fn only_the_first_sample_line_decides_what_a_block_is_about() {
        // The block goes on to show a second variant of the same table, which
        // as one TOML document would be a duplicate key.
        let template = "#: [[accounts]]\n#: name = \"a\"\n#: [accounts.incoming]\n#: protocol = \"imap\"\n#: [accounts.incoming]\n#: protocol = \"jmap\"";
        let anchor = Anchor::of(&lines(template), &Marker::default()).unwrap();
        assert_eq!(anchor.path.to_string(), "accounts");
    }

    #[test]
    fn a_sample_naming_nothing_anchors_nothing() {
        assert!(Anchor::of(&lines("#: not a toml line at all"), &Marker::default()).is_none());
    }

    #[test]
    fn a_value_spanning_several_lines_still_anchors_on_its_key() {
        let anchor = Anchor::of(
            &lines("#: signature = \"\"\"\n#: Jane Doe\n#: Acme Corp\"\"\""),
            &Marker::default(),
        )
        .unwrap();
        assert_eq!(anchor.path.to_string(), "signature");
    }

    #[test]
    fn an_indented_sample_keeps_its_indentation_when_undressed() {
        let marker = Marker::default();
        assert_eq!(marker.undress("#:   layout = \"a\""), "  layout = \"a\"");
        assert_eq!(marker.undress("#: port = 993"), "port = 993");
        assert_eq!(marker.undress("#:"), "");
    }
}
