//! Lining up the `=` of every key in a group.
//!
//! A section is the run of key-value pairs under one `[table]` header, or
//! above the first one. A comment or blank line above a key starts a new group
//! within it. The keys of a group are padded after their name so every `=`
//! sits in one column; a key alone in its group gets one space.
//! A value written over several lines, such as a `'''` string or a multi-line
//! array, is left as it is and sets no width: its `=` is its own affair.

use toml_edit::{Item, Key, Table};

/// Aligns every section under `table`, itself included.
pub(crate) fn align_sections(table: &mut Table) {
    align_section(table);
    align_subsections(table);
}

/// Aligns the sections under `table`. A dotted table belongs to the section
/// that holds it, so only the headed tables below it are reached.
fn align_subsections(table: &mut Table) {
    for (_, item) in table.iter_mut() {
        match item {
            Item::Table(sub) if sub.is_dotted() => align_subsections(sub),
            Item::Table(sub) => align_sections(sub),
            Item::ArrayOfTables(array) => {
                for entry in array.iter_mut() {
                    align_sections(entry);
                }
            }
            _ => {}
        }
    }
}

/// One key-value pair of a section: where it is, and how wide its key renders.
struct Entry {
    /// The names leading to the key, through the dotted tables of the section.
    path: Vec<String>,
    /// Characters from the start of the line to the end of the key text.
    width: usize,
    /// Whether a comment or blank line sits above this key, or above a key
    /// skipped since the previous entry.
    separated: bool,
}

/// Pads the keys directly in `table`, and in the dotted tables under it, so
/// the `=` of each group share a column. Tables with a header of their own are
/// sections of their own and are left for [`align_sections`] to reach.
fn align_section(table: &mut Table) {
    let mut entries = Vec::new();
    let mut separated = false;
    collect(
        table,
        &mut Vec::new(),
        String::new(),
        &mut separated,
        &mut entries,
    );
    let mut groups: Vec<Vec<Entry>> = Vec::new();
    for entry in entries {
        match groups.last_mut() {
            Some(group) if !entry.separated => group.push(entry),
            _ => groups.push(vec![entry]),
        }
    }
    for group in groups {
        let widest = group.iter().map(|entry| entry.width).max().unwrap_or(0);
        for entry in group {
            let Some(mut key) = key_at(table, &entry.path) else {
                continue;
            };
            let suffix = " ".repeat(widest - entry.width + 1);
            key.leaf_decor_mut().set_suffix(suffix);
        }
    }
}

/// Every inline key-value pair of a section, with the text that precedes its
/// `=` measured the way the encoder writes it: the indentation, the key, and
/// each dotted segment with the decor around its dot. `separated` carries a
/// comment or blank line forward to the next entry pushed.
fn collect(
    table: &Table,
    path: &mut Vec<String>,
    above: String,
    separated: &mut bool,
    out: &mut Vec<Entry>,
) {
    for (name, item) in table.iter() {
        let Some(key) = table.key(name) else {
            continue;
        };
        if decor_text(key.leaf_decor().prefix(), "").contains('\n') {
            *separated = true;
        }
        path.push(name.to_owned());
        let mut text = above.clone();
        if path.len() > 1 {
            text.push('.');
            text.push_str(decor_text(key.dotted_decor().prefix(), ""));
        }
        text.push_str(&key.display_repr());
        match item {
            Item::Table(sub) if sub.is_dotted() => {
                text.push_str(decor_text(key.dotted_decor().suffix(), ""));
                collect(sub, path, text, separated, out);
            }
            Item::Value(value) => {
                let dotted_inline = value
                    .as_inline_table()
                    .is_some_and(|inline| inline.is_dotted());
                if !dotted_inline && !value.to_string().contains('\n') {
                    let width = indent(key).chars().count() + text.chars().count();
                    out.push(Entry {
                        path: path.clone(),
                        width,
                        separated: std::mem::take(separated),
                    });
                }
            }
            _ => {}
        }
        path.pop();
    }
}

/// The whitespace between the last newline above a key and the key itself.
fn indent(key: &Key) -> &str {
    let prefix = decor_text(key.leaf_decor().prefix(), "");
    match prefix.rfind('\n') {
        Some(at) => &prefix[at + 1..],
        None => prefix,
    }
}

fn decor_text<'a>(raw: Option<&'a toml_edit::RawString>, default: &'a str) -> &'a str {
    raw.and_then(|raw| raw.as_str()).unwrap_or(default)
}

fn key_at<'t>(table: &'t mut Table, path: &[String]) -> Option<toml_edit::KeyMut<'t>> {
    let (leaf, dotted) = path.split_last()?;
    let mut table = table;
    for name in dotted {
        table = table.get_mut(name)?.as_table_mut()?;
    }
    table.key_mut(leaf)
}

#[cfg(test)]
mod tests {
    use super::*;
    use toml_edit::DocumentMut;

    fn aligned(src: &str) -> String {
        let mut document: DocumentMut = src.parse().unwrap();
        align_sections(document.as_table_mut());
        document.to_string()
    }

    #[test]
    fn pads_keys_to_the_widest_in_the_section() {
        let out = aligned("a = 1\nlonger = 2\n");
        assert_eq!(out, "a      = 1\nlonger = 2\n");
    }

    #[test]
    fn comments_and_blank_lines_start_a_new_group() {
        let out = aligned("a = 1\nbb = 2\n# note\nlonger = 3\nc = 4\n\nd     = 5\n");
        assert_eq!(
            out,
            "a  = 1\nbb = 2\n# note\nlonger = 3\nc      = 4\n\nd = 5\n"
        );
    }

    #[test]
    fn a_comment_above_a_dotted_key_starts_a_new_group() {
        let out = aligned("longer = 1\n# note\na.b = 2\na.cc = 3\n");
        assert_eq!(out, "longer = 1\n# note\na.b  = 2\na.cc = 3\n");
    }

    #[test]
    fn a_comment_above_a_skipped_multiline_value_still_separates() {
        let out = aligned("longer = 1\n# note\ntext = '''\nx\n'''\nb = 2\n");
        assert_eq!(out, "longer = 1\n# note\ntext = '''\nx\n'''\nb = 2\n");
    }

    #[test]
    fn each_header_opens_a_new_span() {
        let out = aligned("a = 1\nbb = 2\n[s]\nlonger = 3\nc = 4\n[[t]]\nx = 1\nyy = 2\n");
        assert_eq!(
            out,
            "a  = 1\nbb = 2\n[s]\nlonger = 3\nc      = 4\n[[t]]\nx  = 1\nyy = 2\n"
        );
    }

    #[test]
    fn dotted_keys_measure_their_whole_path() {
        let out = aligned("a.b = 1\nlonger.key.path = 2\nc = 3\n");
        assert_eq!(
            out,
            "a.b             = 1\nlonger.key.path = 2\nc               = 3\n"
        );
    }

    #[test]
    fn multiline_values_neither_set_nor_take_the_width() {
        let src = "a = 1\nlonger = 2\nmultiline_key = '''\nx\n'''\nlist = [\n  1,\n]\n";
        let out = aligned(src);
        assert_eq!(
            out,
            "a      = 1\nlonger = 2\nmultiline_key = '''\nx\n'''\nlist = [\n  1,\n]\n"
        );
    }

    #[test]
    fn a_lone_key_gets_one_space() {
        assert_eq!(aligned("a   = 1\n"), "a = 1\n");
    }

    #[test]
    fn existing_padding_is_recomputed() {
        assert_eq!(aligned("a        = 1\nbb = 2\n"), "a  = 1\nbb = 2\n");
    }

    #[test]
    fn indentation_counts_toward_the_column() {
        assert_eq!(aligned("  a = 1\nbb = 2\n"), "  a = 1\nbb  = 2\n");
    }
}
