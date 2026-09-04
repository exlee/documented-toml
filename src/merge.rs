//! The merge itself: what runs it, and what comes out.

use std::collections::BTreeSet;

use toml_edit::{ArrayOfTables, Decor, DocumentMut, Item, Key, Table, Value};

use crate::decor::{DefaultEcho, DocBlock, Marker, Prefix, PrefixLine, Sample};
use crate::options::{MergeOptions, ResolvedMigration};
use crate::path::DottedPath;
use crate::report::{Diagnostic, DiagnosticKind, Position, Report, SpanIndex, TomlType};
use crate::source::SourceDocument;
use crate::template::{Template, comment_out};

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
    /// The keys the defaults document without declaring.
    pub(crate) template: Template,
    /// Optional keys the person has not set, written back as `#:` lines and
    /// waiting for something to sit above.
    pub(crate) pending: Vec<String>,
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
            template: Template::default(),
            pending: Vec::new(),
        }
    }

    /// Runs the merge.
    pub(crate) fn run(mut self) -> Merged {
        self.apply_migrations();
        self.template = Template::of(&self.defaults, self.marker());

        let empty = Table::new();
        let defaults = self.defaults.root().unwrap_or(&empty).clone();
        let user = self.user.root().unwrap_or(&empty).clone();

        let mut root = Table::new();
        self.merge_table(&defaults, &user, &mut root, None);
        renumber_tables(&mut root, &mut 0);

        let mut document = DocumentMut::new();
        *document.as_table_mut() = root;
        document.set_trailing(self.merged_trailing());

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
        let optional = self.template.under(path).cloned().unwrap_or_default();
        let mut done: BTreeSet<String> = defaults.iter().map(|(name, _)| name.to_owned()).collect();

        for (name, default_item) in defaults.iter() {
            if matches!(default_item, Item::None) {
                continue;
            }
            let default_key = defaults.key(name).expect("a name iterated has a key");
            let child = child_path(path, name);
            // Anything the defaults documented above this key was written to be
            // read above it, so it goes out first.
            self.merge_optional(&optional, &mut done, user, out, path, Some(&child));
            match user.get_key_value(name) {
                None => {
                    let first = out.is_empty();
                    let (key, item) = self.default_entry(default_key, default_item, &child, first);
                    out.insert_formatted(&key, item);
                }
                Some((user_key, user_item)) => {
                    self.merge_item(default_key, default_item, user_key, user_item, out, &child);
                }
            }
        }

        self.merge_optional(&optional, &mut done, user, out, path, None);

        for (name, user_item) in user.iter() {
            if done.contains(name) || matches!(user_item, Item::None) {
                continue;
            }
            let user_key = user.key(name).expect("a name iterated has a key");
            let child = child_path(path, name);
            if !self.template.knows(&child) {
                let at = self.position(&child);
                self.report.push(Diagnostic::new(
                    DiagnosticKind::UnknownKey,
                    child.clone(),
                    at,
                ));
            }
            let (key, item) = self.keep_unknown(user_key, user_item, &child);
            out.insert_formatted(&key, item);
        }
    }

    /// Writes out the keys the defaults document without declaring, the ones
    /// due at this point in the order.
    ///
    /// A key the person has set merges like any other, its `#:` line being the
    /// default the defaults wrote by hand because there was no live value to
    /// take one from. A key they have not set stays a `#:` line.
    fn merge_optional(
        &mut self,
        optional: &Table,
        done: &mut BTreeSet<String>,
        user: &Table,
        out: &mut Table,
        path: Option<&DottedPath>,
        above: Option<&DottedPath>,
    ) {
        for (name, item) in optional.iter() {
            if done.contains(name) || matches!(item, Item::None) {
                continue;
            }
            let child = child_path(path, name);
            let due = match above {
                Some(next) => self.template.written_before(&child) == Some(next),
                // Whatever is left was written below the last key of its table,
                // or below a key this table does not have.
                None => true,
            };
            if !due {
                continue;
            }
            let Some(key) = optional.key(name) else {
                continue;
            };
            done.insert(name.to_owned());
            match user.get_key_value(name) {
                Some((user_key, user_item)) => {
                    self.optional_set(key, item, user_key, user_item, out, &child);
                }
                None => {
                    // Nothing to sit above yet. It waits for whatever the walk
                    // reaches next, and for the end of the file if nothing.
                    if !self.pending.is_empty() {
                        self.pending.push(String::new());
                    }
                    self.pending
                        .extend(comment_out(&child, key, item, self.marker()));
                }
            }
        }
    }

    /// An optional key the person has set.
    fn optional_set(
        &mut self,
        key: &Key,
        item: &Item,
        user_key: &Key,
        user_item: &Item,
        out: &mut Table,
        path: &DottedPath,
    ) {
        let first = out.is_empty();
        let mut block = DocBlock::default();
        block.keep_user_text(&Prefix::of(decor_of(user_key, user_item)), self.marker());
        let (_, touching) = Prefix::of(decor_of(key, item)).split(self.marker());
        block.take_docs(&touching);
        block.floating = std::mem::take(&mut self.pending);
        if !block.floating.is_empty() {
            block.floating.insert(0, String::new());
            block.floating.push(String::new());
        }

        let mut merged = match (item, user_item) {
            (Item::Value(_), Item::Value(user_value)) => {
                self.record_default(&mut block, key, item, Some(user_value));
                Item::Value(user_value.clone())
            }
            (_, Item::Table(user_table)) => {
                let mut merged = Table::new();
                merged.set_dotted(user_table.is_dotted());
                self.merge_table(&Table::new(), user_table, &mut merged, Some(path));
                Item::Table(merged)
            }
            (_, Item::ArrayOfTables(user_array)) => {
                // The template describes one entry's shape. It documents the
                // first, which is where a person reads what the rest may hold.
                let mut array = user_array.clone();
                if let Some(entry) = array.get_mut(0) {
                    let mut merged = Table::new();
                    self.merge_table(&Table::new(), entry, &mut merged, Some(path));
                    *entry = merged;
                }
                // Entries the template has beyond the first are further
                // examples, not the shape of what is already there. They stay
                // written out, like any optional key nobody has set.
                if let Item::ArrayOfTables(template_array) = item {
                    for extra in template_array.iter().skip(1) {
                        let mut single = ArrayOfTables::new();
                        single.push(extra.clone());
                        if !self.pending.is_empty() {
                            self.pending.push(String::new());
                        }
                        let lines =
                            comment_out(path, key, &Item::ArrayOfTables(single), self.marker());
                        self.pending.extend(lines);
                    }
                }
                Item::ArrayOfTables(array)
            }
            _ => user_item.clone(),
        };

        // A documented key is set apart from the one before it. A key with
        // nothing written above it needs no setting apart.
        let documented = !block.prose.is_empty() || !matches!(block.sample, Sample::None);
        if block.leading_blanks.is_empty() && !first && documented {
            block.leading_blanks = vec![String::new()];
        }

        let mut key = user_key.clone();
        set_prefix(&mut key, &mut merged, block.render(self.marker()));
        out.insert_formatted(&key, merged);
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
        let first = out.is_empty();
        let mut block = self.block(
            decor_of(default_key, default_item),
            decor_of(user_key, user_item),
        );
        self.flush_pending(&mut block, first);
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
            self.record_default(&mut block, default_key, default_item, None);
            user_item.clone()
        } else {
            match (default_item, user_item) {
                (Item::Value(_), Item::Value(user_value)) => {
                    self.record_default(&mut block, default_key, default_item, Some(user_value));
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

    /// Moves any optional keys waiting to be written above whatever comes next.
    ///
    /// `first` says the block opens its table, where there is nothing to be set
    /// apart from. Anywhere else the waiting lines are separated from the key
    /// above them, the way the defaults separated them.
    fn flush_pending(&mut self, block: &mut DocBlock, first: bool) {
        if self.pending.is_empty() {
            return;
        }
        let mut waiting = std::mem::take(&mut self.pending);
        if !first {
            waiting.insert(0, String::new());
        }
        waiting.push(String::new());
        waiting.append(&mut block.floating);
        block.floating = waiting;
    }

    /// The doc block for a key present in both documents: the defaults' standing
    /// text and prose, the person's blanks and comments.
    fn block(&self, default_decor: &Decor, user_decor: &Decor) -> DocBlock {
        let (floating, touching) = Prefix::of(default_decor).split(self.marker());
        let mut block = DocBlock::default();
        block.keep_user_text(&Prefix::of(user_decor), self.marker());
        block.take_docs(&touching);
        block.floating = self.standing_text(&floating);
        if block.leading_blanks.is_empty() && block.floating.is_empty() {
            // The person wrote nothing above this key, so the separation the
            // defaults put there is what keeps the file readable.
            block.leading_blanks = leading_blanks(&floating, &touching);
        }
        block
    }

    /// The defaults' text that stands a blank line away from the key, with the
    /// maintainers' own notes dropped and the blocks that have travelled left
    /// out. Documentation anchored on this key arrives here.
    fn standing_text(&self, floating: &[PrefixLine]) -> Vec<String> {
        let mut out: Vec<String> = Vec::new();
        for run in standing_runs(floating, self.marker()) {
            match run {
                Standing::Blanks(lines) => out.extend(lines),
                Standing::Block(lines) => {
                    let text: Vec<String> = lines
                        .iter()
                        .filter(|line| !matches!(line, PrefixLine::User { .. }))
                        .map(|line| line.text().to_owned())
                        .collect();
                    if self.template.read.contains(&text.join("\n")) {
                        while out.last().is_some_and(|line| line.trim().is_empty()) {
                            out.pop();
                        }
                        continue;
                    }
                    out.extend(text);
                }
            }
        }
        if out.iter().all(|line| line.trim().is_empty()) {
            out.clear();
        }
        out
    }

    /// Records the shipped default above the key when the person's value
    /// differs from it. A generated line replaces whatever samples the defaults
    /// wrote for the same key: both occupy the slot under the prose.
    fn record_default(
        &self,
        block: &mut DocBlock,
        default_key: &Key,
        default_item: &Item,
        user_value: Option<&Value>,
    ) {
        let Item::Value(default_value) = default_item else {
            return;
        };
        let default_text = rendered(default_value);
        if let Some(user_value) = user_value
            && rendered(user_value) == default_text
        {
            return;
        }
        block.sample = Sample::Echo(DefaultEcho {
            key: default_key.display_repr().into_owned(),
            value: default_text,
        });
    }

    // -- keys only on one side -------------------------------------------

    /// A key the person does not have yet, taken from the defaults with the
    /// maintainers' own notes stripped out.
    ///
    /// Optional keys waiting to be written arrive here too. The block above a
    /// key is where the defaults put it, and that is where it goes, whether or
    /// not the person has written the key it belongs above.
    fn default_entry(
        &mut self,
        key: &Key,
        item: &Item,
        path: &DottedPath,
        first: bool,
    ) -> (Key, Item) {
        let prefix = Prefix::of(decor_of(key, item));
        let (floating, touching) = prefix.split(self.marker());
        let mut block = DocBlock {
            leading_blanks: leading_blanks(&floating, &touching),
            indent: prefix.indent().to_owned(),
            ..DocBlock::default()
        };
        block.take_docs(&touching);
        block.floating = self.standing_text(&floating);
        self.flush_pending(&mut block, first);

        let mut key = key.clone();
        let mut item = self.docs_only(item, path);
        set_prefix(&mut key, &mut item, block.render(self.marker()));
        (key, item)
    }

    /// The same stripping, applied through a table or an array of tables.
    ///
    /// An array of tables documents its first entry, which is where a person
    /// reads what the rest may hold, so the optional keys are due there and in
    /// no entry after it.
    fn docs_only(&mut self, item: &Item, path: &DottedPath) -> Item {
        match item {
            Item::Table(table) => Item::Table(self.docs_only_table(table, path, true)),
            Item::ArrayOfTables(array) => {
                let mut out = ArrayOfTables::new();
                for (index, entry) in array.iter().enumerate() {
                    out.push(self.docs_only_table(entry, path, index == 0));
                }
                Item::ArrayOfTables(out)
            }
            other => other.clone(),
        }
    }

    /// The keys of a table the person has not written.
    ///
    /// `documented` walks it as a merge against an empty table, so the keys the
    /// defaults document without declaring are written out at the point in the
    /// order they were written. A person who has not opened a section still
    /// reads what it may hold.
    fn docs_only_table(&mut self, table: &Table, path: &DottedPath, documented: bool) -> Table {
        let mut out = Table::new();
        out.set_dotted(table.is_dotted());
        out.set_implicit(table.is_implicit());
        if documented {
            self.merge_table(table, &Table::new(), &mut out, Some(path));
            return out;
        }
        for (name, item) in table.iter() {
            let key = table.key(name).expect("a name iterated has a key");
            let first = out.is_empty();
            let (key, item) = self.default_entry(key, item, &path.child(name), first);
            out.insert_formatted(&key, item);
        }
        out
    }

    /// A key the defaults do not declare. It is never deleted and never
    /// rewritten, but the marker lines above it go: their key has left the
    /// defaults, so the tool no longer has anything to say about it. Unless the
    /// defaults anchored documentation on it, which arrives here.
    fn keep_unknown(&self, key: &Key, item: &Item, path: &DottedPath) -> (Key, Item) {
        let mut block = DocBlock::default();
        block.keep_user_text(&Prefix::of(decor_of(key, item)), self.marker());
        block.floating = Vec::new();

        let mut key = key.clone();
        let mut item = match item {
            Item::Table(table) => Item::Table(self.keep_unknown_table(table, path)),
            Item::ArrayOfTables(array) => {
                let mut out = ArrayOfTables::new();
                for entry in array.iter() {
                    out.push(self.keep_unknown_table(entry, path));
                }
                Item::ArrayOfTables(out)
            }
            other => other.clone(),
        };
        set_prefix(&mut key, &mut item, block.render(self.marker()));
        (key, item)
    }

    fn keep_unknown_table(&self, table: &Table, path: &DottedPath) -> Table {
        let mut out = Table::new();
        out.set_dotted(table.is_dotted());
        out.set_implicit(table.is_implicit());
        *out.decor_mut() = table.decor().clone();
        for (name, item) in table.iter() {
            let key = table.key(name).expect("a name iterated has a key");
            let (key, item) = self.keep_unknown(key, item, &path.child(name));
            out.insert_formatted(&key, item);
        }
        out
    }

    // -- the end of the file ---------------------------------------------

    /// The text after the last key belongs to no key, so it is not a doc block.
    /// The same ownership holds: the tool's lines there come from the defaults,
    /// and everything else is the person's and is kept as written.
    fn merged_trailing(&mut self) -> String {
        let waiting = std::mem::take(&mut self.pending);
        let marker = self.marker();
        let prefix = Prefix::from_text(self.defaults.trailing());
        let (mut lines, touching) = prefix.split(marker);
        lines.extend(touching);
        let mut docs = self.standing_text(&lines);
        // Optional keys nothing came after belong at the end of the last table
        // they were written in, which is here.
        let mut waiting = waiting;
        if !waiting.is_empty() {
            if !docs.is_empty() {
                waiting.push(String::new());
            }
            waiting.append(&mut docs);
            docs = waiting;
        }
        // One blank line separates the end of the file from the last key,
        // whatever the defaults happened to leave above their own trailing text.
        let first = docs
            .iter()
            .position(|line| !line.trim().is_empty())
            .unwrap_or(docs.len());
        docs.drain(..first);

        let user = Prefix::from_text(self.user.trailing());
        let user_lines = user.lines(marker);
        let first = user_lines
            .iter()
            .position(|line| !matches!(line, PrefixLine::Blank { .. }))
            .unwrap_or(user_lines.len());
        // The tool's lines here are written again from the defaults. Taking
        // them out leaves the blank lines that separated them, which have
        // nothing left to separate.
        let mut mine: Vec<&str> = Vec::new();
        for line in &user_lines[first..] {
            if marker.owns(line.text()) {
                continue;
            }
            let blank = line.text().trim().is_empty();
            if blank && mine.last().is_none_or(|last| last.trim().is_empty()) {
                continue;
            }
            mine.push(line.text());
        }
        while mine.last().is_some_and(|line| line.trim().is_empty()) {
            mine.pop();
        }

        let content = docs.iter().any(|line| !line.trim().is_empty())
            || mine.iter().any(|line| !line.trim().is_empty());
        if !content {
            return String::new();
        }

        let mut out = String::from("\n");
        for line in docs.iter().map(String::as_str).chain(mine) {
            out.push_str(line);
            out.push('\n');
        }
        out
    }
}

/// The blocks in a stretch of standing text.
pub(crate) fn blocks<'a>(lines: &'a [PrefixLine], marker: &Marker) -> Vec<&'a [PrefixLine]> {
    standing_runs(lines, marker)
        .into_iter()
        .filter_map(|run| match run {
            Standing::Block(lines) => Some(lines),
            Standing::Blanks(_) => None,
        })
        .collect()
}

enum Standing<'a> {
    Blanks(Vec<String>),
    Block(&'a [PrefixLine]),
}

/// Standing text split into its blocks, keeping the blank lines between them.
///
/// A blank line ends a block. So does a sample line opening a `[table]`
/// header: it names a key of its own, so what follows documents that key and
/// not the last one. Prose written directly above such a header introduces it,
/// and goes with the block the header opens.
fn standing_runs<'a>(lines: &'a [PrefixLine], marker: &Marker) -> Vec<Standing<'a>> {
    let blank = |line: &PrefixLine| matches!(line, PrefixLine::Blank { .. });
    let mut out = Vec::new();
    let mut at = 0;
    while at < lines.len() {
        let start = at;
        if blank(&lines[at]) {
            while at < lines.len() && blank(&lines[at]) {
                at += 1;
            }
            out.push(Standing::Blanks(
                lines[start..at]
                    .iter()
                    .map(|line| line.text().to_owned())
                    .collect(),
            ));
        } else {
            while at < lines.len() && !blank(&lines[at]) {
                at += 1;
            }
            out.extend(
                split_at_headers(&lines[start..at], marker)
                    .into_iter()
                    .map(Standing::Block),
            );
        }
    }
    out
}

/// One run of the tool's lines, cut where a new `[table]` header takes over.
fn split_at_headers<'a>(run: &'a [PrefixLine], marker: &Marker) -> Vec<&'a [PrefixLine]> {
    let mut starts = vec![0usize];
    for (at, line) in run.iter().enumerate().skip(1) {
        if !opens_table(line, marker) {
            continue;
        }
        let mut start = at;
        while start > 0 && matches!(run[start - 1], PrefixLine::Prose { .. }) {
            start -= 1;
        }
        if start > *starts.last().expect("starts is never empty") {
            starts.push(start);
        }
    }
    starts
        .iter()
        .enumerate()
        .map(|(n, &start)| {
            let end = starts.get(n + 1).copied().unwrap_or(run.len());
            &run[start..end]
        })
        .collect()
}

fn opens_table(line: &PrefixLine, marker: &Marker) -> bool {
    match line {
        PrefixLine::Sample { text } => marker.undress(text).trim_start().starts_with('['),
        _ => false,
    }
}

/// The blank lines a key has above it in the defaults, which is what separates
/// one documented option from the last when the person wrote no blanks of their
/// own.
fn leading_blanks(floating: &[PrefixLine], touching: &[PrefixLine]) -> Vec<String> {
    if !floating.is_empty() && touching.iter().any(|line| !line.text().trim().is_empty()) {
        return vec![String::new()];
    }
    Vec::new()
}

fn child_path(path: Option<&DottedPath>, name: &str) -> DottedPath {
    match path {
        Some(path) => path.child(name),
        None => DottedPath::new(name),
    }
}

/// Where the text above a key lives.
///
/// For a key-value pair it is the key's own leaf decor. A standalone table
/// carries it in its own decor, and an array of tables in that of its first
/// entry, because that is what sits under the comment in the file.
pub(crate) fn decor_of<'a>(key: &'a Key, item: &'a Item) -> &'a Decor {
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

// -- path navigation, for migrations and anchors ------------------------

fn lookup<'t>(root: &'t Table, path: &DottedPath) -> Option<&'t Item> {
    let mut table = root;
    let mut segments = path.segments().peekable();
    while let Some(segment) = segments.next() {
        let item = table.get(segment)?;
        if segments.peek().is_none() {
            return Some(item);
        }
        table = match item {
            Item::Table(sub) => sub,
            Item::ArrayOfTables(array) => array.get(0)?,
            _ => return None,
        };
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
