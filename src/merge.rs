//! The merge itself: what runs it, and what comes out.

use toml_edit::{ArrayOfTables, Decor, DocumentMut, Item, Key, Table, Value};

use crate::decor::{DefaultEcho, DocBlock, Marker, Prefix};
use crate::options::{MergeOptions, ResolvedMigration};
use crate::path::DottedPath;
use crate::report::{Diagnostic, DiagnosticKind, Position, Report, SpanIndex, TomlType};
use crate::source::SourceDocument;

/// The merged document and its report.
///
/// This is also the effective configuration. Every default key is materialised
/// with a live value, so the file written back and the configuration a caller
/// deserializes cannot drift apart.
#[derive(Debug, Clone)]
pub struct Merged {
    /// Everything the merge noticed.
    pub report: Report,
    pub(crate) document: DocumentMut,
}

impl Merged {
    /// The merged document, for callers that want to deserialize it.
    pub fn document(&self) -> &DocumentMut {
        &self.document
    }

    /// The merged document, taken by value.
    pub fn into_document(self) -> DocumentMut {
        self.document
    }

    /// The file to write back.
    pub fn to_toml_string(&self) -> String {
        self.document.to_string()
    }
}

/// Walks the default document as the spine and transplants user values.
///
/// The defaults give the shape and the order; the user gives the values.
#[derive(Debug, Clone)]
pub struct MergeEngine {
    /// The shipped document, which gives the shape and the order.
    pub(crate) defaults: SourceDocument,
    /// The person's document, which gives the values.
    pub(crate) user: SourceDocument,
    /// The text the spans point into, needed to turn a byte offset into a line
    /// and a column.
    pub(crate) user_src: String,
    pub(crate) options: MergeOptions,
    pub(crate) migrations: Vec<ResolvedMigration>,
    pub(crate) spans: SpanIndex,
    pub(crate) report: Report,
}

impl MergeEngine {
    pub(crate) fn new(
        defaults: SourceDocument,
        user: SourceDocument,
        user_src: &str,
        options: MergeOptions,
        migrations: Vec<ResolvedMigration>,
        spans: SpanIndex,
    ) -> Self {
        Self {
            defaults,
            user,
            user_src: user_src.to_owned(),
            options,
            migrations,
            spans,
            report: Report::default(),
        }
    }

    /// Runs the merge.
    pub(crate) fn run(mut self) -> Merged {
        self.apply_migrations();

        let empty = Table::new();
        let defaults = self.defaults.root().unwrap_or(&empty).clone();
        let user = self.user.root().unwrap_or(&empty).clone();

        let mut root = Table::new();
        self.merge_table(&defaults, &user, &mut root, None);
        renumber_tables(&mut root, &mut 0);

        let mut document = DocumentMut::new();
        *document.as_table_mut() = root;
        document.set_trailing(self.user.trailing().to_owned());

        Merged {
            report: self.report,
            document,
        }
    }

    fn marker(&self) -> &Marker {
        &self.options.marker
    }

    /// Where a key sits in the person's file.
    fn position(&self, path: &DottedPath) -> Position {
        self.spans.position(&self.user_src, path)
    }

    // -- migrations ------------------------------------------------------

    /// Moves values the rename rules name, before the merge walk, so a migrated
    /// value takes part in the merge as if the person had written it in place.
    fn apply_migrations(&mut self) {
        let migrations = std::mem::take(&mut self.migrations);
        for migration in &migrations {
            let Some(root) = self.user.root_mut() else {
                break;
            };
            if lookup(root, &migration.to).is_some() {
                // The user's explicit new-path value wins; the old path falls
                // through to the unknown-key rule.
                continue;
            }
            let Some((key, item)) = take(root, &migration.from) else {
                continue;
            };
            let renamed = Key::new(migration.to.leaf()).with_leaf_decor(key.leaf_decor().clone());
            if !place(root, &migration.to, renamed, item) {
                continue;
            }
            let at = self.position(&migration.from);
            self.report.push(Diagnostic::new(
                DiagnosticKind::Migrated {
                    from: migration.from.clone(),
                },
                migration.to.clone(),
                at,
            ));
        }
        self.migrations = migrations;
    }

    // -- the walk --------------------------------------------------------

    fn merge_table(
        &mut self,
        defaults: &Table,
        user: &Table,
        out: &mut Table,
        path: Option<&DottedPath>,
    ) {
        for (name, default_item) in defaults.iter() {
            if matches!(default_item, Item::None) {
                continue;
            }
            let default_key = defaults.key(name).expect("a name iterated has a key");
            let child = child_path(path, name);
            match user.get_key_value(name) {
                None => {
                    let (key, item) = self.default_entry(default_key, default_item);
                    out.insert_formatted(&key, item);
                }
                Some((user_key, user_item)) => {
                    self.merge_item(default_key, default_item, user_key, user_item, out, &child);
                }
            }
        }

        for (name, user_item) in user.iter() {
            if defaults.contains_key(name) || matches!(user_item, Item::None) {
                continue;
            }
            let user_key = user.key(name).expect("a name iterated has a key");
            let child = child_path(path, name);
            let at = self.position(&child);
            self.report
                .push(Diagnostic::new(DiagnosticKind::UnknownKey, child, at));
            let (key, item) = self.keep_unknown(user_key, user_item);
            out.insert_formatted(&key, item);
        }
    }

    /// Merges one key present in both documents.
    fn merge_item(
        &mut self,
        default_key: &Key,
        default_item: &Item,
        user_key: &Key,
        user_item: &Item,
        out: &mut Table,
        path: &DottedPath,
    ) {
        let mut block = self.block(
            decor_of(default_key, default_item),
            decor_of(user_key, user_item),
        );
        let expected = TomlType::of(default_item);
        let found = TomlType::of(user_item);

        let mut item = if expected != found {
            if let (Some(expected), Some(found)) = (expected, found) {
                let at = self.position(path);
                self.report.push(Diagnostic::new(
                    DiagnosticKind::TypeMismatch { expected, found },
                    path.clone(),
                    at,
                ));
            }
            // The person's value stays exactly as they wrote it. Only the block
            // above it is rebuilt, because that text belongs to the tool.
            block.echo = self.echo(default_key, default_item, None);
            user_item.clone()
        } else {
            match (default_item, user_item) {
                (Item::Value(_), Item::Value(user_value)) => {
                    block.echo = self.echo(default_key, default_item, Some(user_value));
                    Item::Value(user_value.clone())
                }
                (Item::Table(default_table), Item::Table(user_table)) => {
                    let mut merged = Table::new();
                    merged.set_dotted(user_table.is_dotted());
                    merged.set_implicit(default_table.is_implicit() && user_table.is_implicit());
                    self.merge_table(default_table, user_table, &mut merged, Some(path));
                    Item::Table(merged)
                }
                (Item::ArrayOfTables(_), Item::ArrayOfTables(user_array)) => {
                    // The person's set is complete: the defaults contribute no
                    // entries and no fields. Only the documentation is refreshed.
                    Item::ArrayOfTables(user_array.clone())
                }
                _ => unreachable!("equal TOML types have matching item shapes"),
            }
        };

        let mut key = user_key.clone();
        set_prefix(&mut key, &mut item, block.render(self.marker()));
        out.insert_formatted(&key, item);
    }

    /// The doc block for a key present in both documents: the person's blanks
    /// and comments, the defaults' documentation.
    fn block(&self, default_decor: &Decor, user_decor: &Decor) -> DocBlock {
        let mut block = DocBlock::default();
        block.keep_user_text(&Prefix::of(user_decor), self.marker());
        block.take_docs(&Prefix::of(default_decor), self.marker());
        block
    }

    /// The line recording the shipped default, when the person's value differs
    /// from it. A value equal to the default needs no echo, and neither does a
    /// key that has no comparable value at all.
    fn echo(
        &self,
        default_key: &Key,
        default_item: &Item,
        user_value: Option<&Value>,
    ) -> Option<DefaultEcho> {
        let Item::Value(default_value) = default_item else {
            return None;
        };
        let default_text = rendered(default_value);
        if let Some(user_value) = user_value
            && rendered(user_value) == default_text
        {
            return None;
        }
        Some(DefaultEcho {
            key: default_key.display_repr().into_owned(),
            value: default_text,
        })
    }

    // -- keys only on one side -------------------------------------------

    /// A key the person does not have yet, taken from the defaults with the
    /// maintainers' own notes stripped out.
    fn default_entry(&self, key: &Key, item: &Item) -> (Key, Item) {
        let prefix = Prefix::of(decor_of(key, item));
        let mut block = DocBlock {
            leading_blanks: leading_blanks(&prefix, self.marker()),
            indent: prefix.indent().to_owned(),
            ..DocBlock::default()
        };
        block.take_docs(&prefix, self.marker());

        let mut key = key.clone();
        let mut item = self.docs_only(item);
        set_prefix(&mut key, &mut item, block.render(self.marker()));
        (key, item)
    }

    /// The same stripping, applied through a table or an array of tables.
    fn docs_only(&self, item: &Item) -> Item {
        match item {
            Item::Table(table) => Item::Table(self.docs_only_table(table)),
            Item::ArrayOfTables(array) => {
                let mut out = ArrayOfTables::new();
                for entry in array.iter() {
                    out.push(self.docs_only_table(entry));
                }
                Item::ArrayOfTables(out)
            }
            other => other.clone(),
        }
    }

    fn docs_only_table(&self, table: &Table) -> Table {
        let mut out = Table::new();
        out.set_dotted(table.is_dotted());
        out.set_implicit(table.is_implicit());
        if !table.is_dotted() {
            let prefix = Prefix::of(table.decor());
            let mut block = DocBlock {
                leading_blanks: leading_blanks(&prefix, self.marker()),
                ..DocBlock::default()
            };
            block.take_docs(&prefix, self.marker());
            out.decor_mut().set_prefix(block.render(self.marker()));
        }
        for (name, item) in table.iter() {
            let key = table.key(name).expect("a name iterated has a key");
            let (key, item) = self.default_entry(key, item);
            out.insert_formatted(&key, item);
        }
        out
    }

    /// A key the defaults do not declare. It is never deleted and never
    /// rewritten, but the marker lines above it go: their key has left the
    /// defaults, so the tool no longer has anything to say about it.
    fn keep_unknown(&self, key: &Key, item: &Item) -> (Key, Item) {
        let mut block = DocBlock::default();
        block.keep_user_text(&Prefix::of(decor_of(key, item)), self.marker());

        let mut key = key.clone();
        let mut item = match item {
            Item::Table(table) => Item::Table(self.keep_unknown_table(table)),
            Item::ArrayOfTables(array) => {
                let mut out = ArrayOfTables::new();
                for entry in array.iter() {
                    out.push(self.keep_unknown_table(entry));
                }
                Item::ArrayOfTables(out)
            }
            other => other.clone(),
        };
        set_prefix(&mut key, &mut item, block.render(self.marker()));
        (key, item)
    }

    fn keep_unknown_table(&self, table: &Table) -> Table {
        let mut out = Table::new();
        out.set_dotted(table.is_dotted());
        out.set_implicit(table.is_implicit());
        *out.decor_mut() = table.decor().clone();
        for (name, item) in table.iter() {
            let key = table.key(name).expect("a name iterated has a key");
            let (key, item) = self.keep_unknown(key, item);
            out.insert_formatted(&key, item);
        }
        out
    }
}

/// Where the text above a key lives.
///
/// For a key-value pair it is the key's own leaf decor. A standalone table
/// carries it in its own decor, and an array of tables in that of its first
/// entry, because that is what sits under the comment in the file.
fn decor_of<'a>(key: &'a Key, item: &'a Item) -> &'a Decor {
    match item {
        Item::Table(table) if !table.is_dotted() => table.decor(),
        Item::ArrayOfTables(array) => match array.get(0) {
            Some(first) => first.decor(),
            None => key.leaf_decor(),
        },
        _ => key.leaf_decor(),
    }
}

/// Writes a rebuilt block back where [`decor_of`] found it, leaving nothing
/// behind in the place it did not use.
fn set_prefix(key: &mut Key, item: &mut Item, prefix: String) {
    match item {
        Item::Table(table) if !table.is_dotted() => {
            key.leaf_decor_mut().set_prefix("");
            table.decor_mut().set_prefix(prefix);
        }
        Item::ArrayOfTables(array) => {
            key.leaf_decor_mut().set_prefix("");
            if let Some(first) = array.get_mut(0) {
                first.decor_mut().set_prefix(prefix);
            }
        }
        _ => {
            key.leaf_decor_mut().set_prefix(prefix);
        }
    }
}

fn child_path(path: Option<&DottedPath>, name: &str) -> DottedPath {
    match path {
        Some(path) => path.child(name),
        None => DottedPath::new(name),
    }
}

fn leading_blanks(prefix: &Prefix, marker: &Marker) -> Vec<String> {
    let mut block = DocBlock::default();
    block.keep_user_text(prefix, marker);
    block.leading_blanks
}

/// A value's text with every scrap of formatting whitespace removed, which is
/// what "does the user's value differ from the default" compares. Comparing the
/// text keeps float equality out of it and treats `1_000` and `1000` as the
/// different text they are.
fn rendered(value: &Value) -> String {
    let mut value = value.clone();
    strip(&mut value);
    value.to_string().trim().to_owned()
}

fn strip(value: &mut Value) {
    match value {
        Value::Array(array) => {
            for element in array.iter_mut() {
                strip(element);
            }
            array.set_trailing("");
            array.set_trailing_comma(false);
        }
        Value::InlineTable(table) => {
            for (mut key, element) in table.iter_mut() {
                key.leaf_decor_mut().clear();
                key.dotted_decor_mut().clear();
                strip(element);
            }
            table.set_trailing("");
            table.set_trailing_comma(false);
        }
        _ => {}
    }
    value.decor_mut().clear();
}

/// Renumbers standalone tables in emission order.
///
/// `DocumentMut` sorts tables by their recorded position, and a table
/// transplanted from either input carries a position from a document it is no
/// longer in. Numbering them as the walk reaches them makes the output follow
/// the defaults' declaration order.
fn renumber_tables(table: &mut Table, next: &mut isize) {
    for (_, item) in table.iter_mut() {
        match item {
            Item::Table(sub) => {
                if !sub.is_dotted() {
                    *next += 1;
                    sub.set_position(Some(*next));
                }
                renumber_tables(sub, next);
            }
            Item::ArrayOfTables(array) => {
                for entry in array.iter_mut() {
                    *next += 1;
                    entry.set_position(Some(*next));
                    renumber_tables(entry, next);
                }
            }
            _ => {}
        }
    }
}

// -- path navigation, for migrations ------------------------------------

fn lookup<'t>(root: &'t Table, path: &DottedPath) -> Option<&'t Item> {
    let mut table = root;
    let mut segments = path.segments().peekable();
    while let Some(segment) = segments.next() {
        let item = table.get(segment)?;
        if segments.peek().is_none() {
            return Some(item);
        }
        table = item.as_table()?;
    }
    None
}

fn take(root: &mut Table, path: &DottedPath) -> Option<(Key, Item)> {
    let mut table = root;
    let mut segments = path.segments().peekable();
    while let Some(segment) = segments.next() {
        if segments.peek().is_none() {
            return table.remove_entry(segment);
        }
        table = table.get_mut(segment)?.as_table_mut()?;
    }
    None
}

/// Inserts at a path, creating the tables above it as implicit ones. Answers
/// whether the path could be reached at all.
fn place(root: &mut Table, path: &DottedPath, key: Key, item: Item) -> bool {
    let mut table = root;
    let mut segments = path.segments().peekable();
    while let Some(segment) = segments.next() {
        if segments.peek().is_none() {
            table.insert_formatted(&key, item);
            return true;
        }
        let entry = table.entry(segment).or_insert_with(|| {
            let mut created = Table::new();
            created.set_implicit(true);
            Item::Table(created)
        });
        match entry.as_table_mut() {
            Some(sub) => table = sub,
            None => return false,
        }
    }
    false
}
