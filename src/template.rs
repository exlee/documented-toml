//! The keys the defaults document without declaring.
//!
//! A `#:` line is TOML, so a run of them is a document: the keys an
//! application cannot ship a value for, written out as the person would write
//! them. Read back into a table, with the `##:` prose above each key kept as
//! that key's own comment, they merge exactly like keys the defaults declare.
//! The one difference is a key the person has not set: it stays a `#:` line
//! instead of becoming a live value.

use std::collections::{BTreeMap, BTreeSet};

use toml_edit::{DocumentMut, Item, Key, Table};

use crate::decor::{Marker, Prefix, PrefixLine};
use crate::path::DottedPath;
use crate::source::SourceDocument;

/// Every key the defaults document but do not declare, under the path it would
/// be written at.
#[derive(Debug, Clone, Default)]
pub struct Template {
    pub(crate) root: Table,
    /// Blocks that could not be read as TOML, by their text. They stay where
    /// the defaults wrote them.
    pub(crate) unread: BTreeSet<String>,
    /// Blocks that were read, by their text, so the walk that emits the
    /// defaults' standing text leaves them out.
    pub(crate) read: BTreeSet<String>,
    /// The key each block was written above, so an optional key keeps the
    /// place in the order the defaults gave it. Absent means it was written
    /// below the last key of its table.
    pub(crate) written_above: BTreeMap<DottedPath, DottedPath>,
}

impl Template {
    /// Reads the standing text of a defaults document.
    pub fn of(defaults: &SourceDocument, marker: &Marker) -> Self {
        let mut template = Self::default();
        let Some(root) = defaults.root() else {
            return template;
        };
        let mut section = None;
        template.walk(root, None, &mut section, marker);

        let prefix = Prefix::from_text(defaults.trailing());
        let (mut lines, touching) = prefix.split(marker);
        lines.extend(touching);
        template.read_region(&lines, section.as_ref(), None, marker);
        template
    }

    /// Walks the defaults in the order they are written, tracking which table's
    /// section the text between keys belongs to.
    fn walk(
        &mut self,
        table: &Table,
        within: Option<&DottedPath>,
        section: &mut Option<DottedPath>,
        marker: &Marker,
    ) {
        for (name, item) in table.iter() {
            let Some(key) = table.key(name) else {
                continue;
            };
            let child = match within {
                Some(path) => path.child(name),
                None => DottedPath::new(name),
            };
            // Text above a `[table]` header closes the section before it, so it
            // is read against the table it follows, not the one it precedes.
            let reading = match item {
                Item::Table(sub) if !sub.is_dotted() => section.clone(),
                Item::ArrayOfTables(_) => section.clone(),
                _ => within.cloned(),
            };
            let (floating, _) = Prefix::of(crate::merge::decor_of(key, item)).split(marker);
            self.read_region(&floating, reading.as_ref(), Some(&child), marker);

            if let Item::Table(sub) = item {
                if !sub.is_dotted() {
                    *section = Some(child.clone());
                }
                self.walk(sub, Some(&child), section, marker);
            }
        }
    }

    /// Reads one stretch of standing text, block by block.
    ///
    /// A block opening a `[table]` header moves the section the blocks after it
    /// are read against, the way the header would in a file of its own.
    fn read_region(
        &mut self,
        lines: &[PrefixLine],
        within: Option<&DottedPath>,
        above: Option<&DottedPath>,
        marker: &Marker,
    ) {
        let mut scaffold = within.cloned();
        for block in crate::merge::blocks(lines, marker) {
            if let Some(header) = header_of(block, marker) {
                scaffold = Some(header);
            }
            let text = block
                .iter()
                .filter(|line| !matches!(line, PrefixLine::User { .. }))
                .map(|line| line.text().to_owned())
                .collect::<Vec<_>>()
                .join("\n");
            if self.read_block(block, scaffold.as_ref(), above, marker) {
                self.read.insert(text);
            } else {
                self.unread.insert(text);
            }
        }
    }

    /// Reads one block, answering whether it could be read at all.
    ///
    /// A block is left where it was written when it is not TOML, when it names
    /// a key another block has already named, or when it ends in prose that
    /// introduces no key. The first two are a defaults author showing two ways
    /// to fill in one table, which means both to be read, not merged.
    fn read_block(
        &mut self,
        block: &[PrefixLine],
        scaffold: Option<&DottedPath>,
        above: Option<&DottedPath>,
        marker: &Marker,
    ) -> bool {
        if !block
            .iter()
            .any(|line| matches!(line, PrefixLine::Sample { .. }))
        {
            return false;
        }
        let Ok(document) = source_of(block, scaffold, marker).parse::<DocumentMut>() else {
            return false;
        };
        // Prose introduces what follows it. Prose with nothing following it
        // introduces nothing, and reading the block would drop it, so the block
        // is left where it was written instead.
        if document
            .trailing()
            .as_str()
            .is_some_and(|text| !text.trim().is_empty())
        {
            return false;
        }
        let mut merged = self.root.clone();
        if !graft(&mut merged, document.as_table()) {
            return false;
        }
        self.root = merged;
        if let Some(above) = above {
            let mut paths = Vec::new();
            collect_paths(document.as_table(), None, &mut paths);
            for path in paths {
                self.written_above
                    .entry(path)
                    .or_insert_with(|| above.clone());
            }
        }
        true
    }

    /// The key an optional key was written above, if it was written above one.
    pub(crate) fn written_before(&self, path: &DottedPath) -> Option<&DottedPath> {
        self.written_above.get(path)
    }

    /// Whether the template names a key, so a person setting it is not being
    /// told about an option that does not exist.
    pub(crate) fn knows(&self, path: &DottedPath) -> bool {
        let mut table = &self.root;
        let mut segments = path.segments().peekable();
        while let Some(segment) = segments.next() {
            let Some(item) = table.get(segment) else {
                return false;
            };
            if segments.peek().is_none() {
                return true;
            }
            table = match item {
                Item::Table(sub) => sub,
                Item::ArrayOfTables(array) => match array.get(0) {
                    Some(first) => first,
                    None => return false,
                },
                _ => return false,
            };
        }
        false
    }

    /// The keys this template holds under a path, if any.
    pub(crate) fn under(&self, path: Option<&DottedPath>) -> Option<&Table> {
        let Some(path) = path else {
            return Some(&self.root);
        };
        let mut table = &self.root;
        for segment in path.segments() {
            table = match table.get(segment)? {
                Item::Table(sub) => sub,
                Item::ArrayOfTables(array) => array.get(0)?,
                _ => return None,
            };
        }
        Some(table)
    }
}

/// The block's source as TOML: sample lines undressed, prose kept as the
/// comments they already are, under the header of the section it sits in so a
/// bare assignment lands where it was written.
fn source_of(block: &[PrefixLine], scaffold: Option<&DottedPath>, marker: &Marker) -> String {
    let mut source = String::new();
    if let Some(path) = scaffold
        && header_of(block, marker).is_none()
    {
        source.push_str(&format!("[{path}]\n"));
    }
    for line in block {
        match line {
            PrefixLine::Sample { text } => source.push_str(&marker.undress(text)),
            PrefixLine::Prose { text } => source.push_str(text.trim_start()),
            PrefixLine::Blank { text } => source.push_str(text),
            PrefixLine::User { .. } => continue,
        }
        source.push('\n');
    }
    source
}

/// Every key path a parsed block declares.
fn collect_paths(table: &Table, within: Option<&DottedPath>, out: &mut Vec<DottedPath>) {
    for (name, item) in table.iter() {
        let child = match within {
            Some(path) => path.child(name),
            None => DottedPath::new(name),
        };
        out.push(child.clone());
        match item {
            Item::Table(sub) => collect_paths(sub, Some(&child), out),
            Item::ArrayOfTables(array) => {
                for entry in array.iter() {
                    collect_paths(entry, Some(&child), out);
                }
            }
            _ => {}
        }
    }
}

/// Puts a blank line above every table nested in an item, the way a person
/// writing the block out by hand would. A table carrying prose keeps it, with
/// the blank line above that.
fn space_tables(item: &mut Item) {
    let tables: Vec<&mut Table> = match item {
        Item::Table(table) => vec![table],
        Item::ArrayOfTables(array) => {
            // Every entry after the first opens a header of its own.
            for entry in array.iter_mut().skip(1) {
                space(entry.decor_mut());
            }
            array.iter_mut().collect()
        }
        _ => return,
    };
    for table in tables {
        for (_, nested) in table.iter_mut() {
            match nested {
                Item::Table(sub) => space(sub.decor_mut()),
                Item::ArrayOfTables(array) => {
                    for entry in array.iter_mut() {
                        space(entry.decor_mut());
                    }
                }
                _ => continue,
            }
            space_tables(nested);
        }
    }
}

fn space(decor: &mut toml_edit::Decor) {
    let prefix = decor
        .prefix()
        .and_then(|raw| raw.as_str())
        .unwrap_or_default()
        .to_owned();
    if prefix.trim().is_empty() {
        // Nothing to keep, so the encoder's own blank line before a header is
        // what the block wants.
        decor.clear();
    } else if !prefix.starts_with('\n') {
        decor.set_prefix(format!("\n{prefix}"));
    }
}

/// The key path of the first `[table]` or `[[array]]` header in a block.
fn header_of(block: &[PrefixLine], marker: &Marker) -> Option<DottedPath> {
    let body = block.iter().find_map(|line| match line {
        PrefixLine::Sample { text } => {
            let body = marker.undress(text);
            body.trim_start().starts_with('[').then_some(body)
        }
        _ => None,
    })?;
    let body = body.trim();
    let inner = body
        .strip_prefix("[[")
        .and_then(|rest| rest.split("]]").next())
        .or_else(|| {
            body.strip_prefix('[')
                .and_then(|rest| rest.split(']').next())
        })?;
    DottedPath::parse(inner).ok()
}

/// Adds one block's keys to the template, answering whether they all fit.
///
/// Tables combine; anything already holding a value does not.
fn graft(into: &mut Table, from: &Table) -> bool {
    for (name, item) in from.iter() {
        let Some(key) = from.key(name) else {
            continue;
        };
        match (into.get_mut(name), item) {
            (None, _) => {
                into.insert_formatted(key, item.clone());
            }
            (Some(Item::Table(there)), Item::Table(here)) => {
                if !graft(there, here) {
                    return false;
                }
            }
            // A bare block written under `[accounts]` where the template holds
            // an `[[accounts]]` belongs in the entry that is already there.
            (Some(Item::ArrayOfTables(there)), Item::Table(here)) => match there.get_mut(0) {
                Some(first) => {
                    if !graft(first, here) {
                        return false;
                    }
                }
                None => return false,
            },
            // A second `[[entry]]` block extends the array, as it would in a
            // file: it is another example of what one entry may hold.
            (Some(Item::ArrayOfTables(there)), Item::ArrayOfTables(here)) => {
                for entry in here.iter() {
                    there.push(entry.clone());
                }
            }
            _ => return false,
        }
    }
    true
}

/// One key of the template written back as the `#:` lines it came from.
///
/// A table comes with its header, because that is how a person would write it.
/// A bare assignment does not: it is written where the table it belongs to is
/// already open. The prose sitting above the key in the template comes with it
/// either way.
pub(crate) fn comment_out(
    path: &DottedPath,
    key: &Key,
    item: &Item,
    marker: &Marker,
) -> Vec<String> {
    let mut document = DocumentMut::new();
    let mut table = document.as_table_mut();
    let mut segments: Vec<&str> = if matches!(item, Item::Value(_)) {
        Vec::new()
    } else {
        path.segments().collect()
    };
    segments.pop();
    for segment in segments {
        let entry = table.entry(segment).or_insert_with(|| {
            let mut created = Table::new();
            created.set_implicit(true);
            Item::Table(created)
        });
        let Some(sub) = entry.as_table_mut() else {
            return Vec::new();
        };
        table = sub;
    }
    let mut item = item.clone();
    space_tables(&mut item);
    table.insert_formatted(key, item);

    document
        .to_string()
        .lines()
        .map(|line| {
            if marker.owns(line) || line.trim().is_empty() && line.is_empty() {
                line.to_owned()
            } else if line.trim().is_empty() {
                marker.sample().to_owned()
            } else {
                format!("{} {line}", marker.sample())
            }
        })
        .collect()
}
