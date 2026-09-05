# documented-toml design

## 1. Purpose

`documented-toml` reconciles two TOML documents:

- the **defaults**: the configuration an application ships, with documentation
  written alongside each option;
- the **user file**: the same configuration after a person has edited it.

The output is a single TOML document that a person can keep editing. It holds
the user's values, the user's own comments, and every option the defaults
currently declare, including options added after the user last opened the file.
Alongside the document, the merge produces a report describing anything the
application may want to act on: keys it does not recognise, values of the wrong
type, and keys moved by a rename rule.

The merge is format-preserving. Comments, key order, spacing and value
formatting from the user's file survive it.

## 2. Why a dedicated crate

`toml` with serde parses into typed values and drops everything that is not
data: comments, blank lines, key order, whether a number was written `1_000` or
`1000`. Writing that back replaces a file a person has curated with a machine
rendering of it.

`figment` and `config` layer several sources into one effective configuration in
memory. They answer "what is the value of `timeout`". They do not answer "what
should this person's config file look like now that the application gained three
options".

`toml_edit` provides the piece both of those lack: a document object model that
retains the original text of everything it did not change. It is the foundation
here. `documented-toml` adds the merge policy, the documentation-comment convention,
and the report.

## 3. Vocabulary

**Default document** the shipped configuration, parsed from the application's
own source. Authored by the application's maintainers.

**User document** the file on the person's disk.

**Merged document** the output. It is both the file to write back and the
effective configuration, because every default key is materialised into it with
a live value. A caller that needs typed access deserializes the merged document
and needs no second layering pass.

**Marker** a comment prefix that identifies text owned by the tool. There are
two: `##:` for prose and `#:` for TOML text. Both configurable.

**Doc block** the run of marker lines that sits immediately above a key.

**Sample** a `#:` line. It is TOML, not English: the record of a shipped
default, or an example of an option the defaults ship no value for.

**Anchor** the key a block's first sample line names. It says what the block
documents, and where the block belongs.

**Standing text** marker lines the defaults keep a blank line away from any
key, including the text after the last key.

## 4. The comment convention

Borrowed from kitty's `kitty.conf`. Three kinds of comment coexist in a user's
file and are told apart by their prefix:

```toml
##: Request timeout in seconds.
##: Values above 300 are clamped.
#: timeout = 30
# bumped for ticket 44
timeout = 120
```

Lines starting with `##:` are the tool's prose: the sentences that explain an
option. Lines starting with `#:` are the tool's TOML text. Both are rewritten
from the defaults on every merge and removed when their key leaves the
defaults. Anything a person writes there is lost on the next merge, which is
the price of having documentation that stays current.

Lines starting with a plain `#` belong to the person. The merge never rewrites,
reflows or reorders them. They travel with their key.

A line carries a marker when its first non-whitespace characters are exactly
that marker and its run of `#` is the same length as the marker's. `##:` is
prose and `#:` is TOML text; `####:` and `#` are the person's.

Prose and TOML text are separated because they are read differently. Prose is
for the eye. A `#:` line is a line of the file, one the person can uncomment
and edit, and one the tool can read.

### 4.1 In the default document

Only marker lines propagate. A plain `#` comment in the defaults is a note for
whoever maintains the defaults and never reaches a user's file.

```toml
# TODO: revisit before 1.0        <- stays in the defaults
##: Request timeout in seconds.   <- reaches the user's file
timeout = 30
```

A comment inside a `#:` line is part of the TOML text and travels with it, the
way any comment inside a sample would.

### 4.2 Recording the shipped default

When the user's value differs from the shipped default, the merge records the
default above the key as a sample:

```toml
##: Request timeout in seconds.
#: timeout = 30
timeout = 120
```

The person can see what they departed from without opening the application's
source. When the value equals the default, or the key was absent and the merge
inserted it, no such line is written: the live value already is the default.

The line is TOML, so it names its key the way the line under it does. A key the
person wrote dotted keeps every segment the `[table]` header above does not
supply, and the person's spelling is the one that counts, the merged file being
theirs:

```toml
[s]
##: How long to wait.
#: a.b = 30
a.b = 5
```

A generated line takes the sample slot, replacing whatever samples the defaults
wrote in the same block. A sample for a different option belongs in a block of
its own, a blank line away.

### 4.3 Block assembly

The text above a key is rebuilt in this order:

1. standing text the defaults kept a blank line away from the key, verbatim;
2. blank lines above the block;
3. the prose from the current defaults;
4. the sample: the shipped default when the value was overridden, otherwise
   whatever samples the defaults wrote;
5. the user's own `#` lines, in their original order;
6. the key.

The user's notes sit closest to the key they annotate; the refreshable block
sits above them.

The blank lines in step 2 are the user's when they wrote any. When they wrote
none, the blank line the defaults put above the key is used, because that
separation is part of the shape the defaults give the file and options with a
paragraph of prose each are unreadable run together.

### 4.4 Optional keys

A `#:` line is TOML, so a run of them is a document: the options an application
cannot ship a value for, written out as the person would write them.

```toml
##: How long to wait. Unset, the server decides.
#: timeout = 30

##: Accounts are written as [[accounts]] blocks:
#: [[accounts]]
#: name = "Personal"
#: host = "imap.example.com"
```

These are **optional keys**: keys the defaults document without declaring. Read
back into a table, with the `##:` prose above each one kept as that key's own
comment, they take part in the merge exactly as declared keys do. Section 5's
rules apply to them unchanged, and two things follow.

A person who sets one is not told the option does not exist. The defaults know
about it; they only had no value to give it.

Set, it merges like any other key, and its `#:` line is the shipped default
recorded above it under section 4.2. For a declared key that line is generated
from the live value; for an optional key the defaults wrote it out by hand,
because there was no live value to take one from. Same line, same meaning, same
place.

```toml
# defaults                    # user                  # merged
##: This value is counter     counter = 3             ##: This value is counter
counter = 1                   optional_counter = 5    #: counter = 1
                                                      counter = 3
##: Optional counter
#: optional_counter = 1                               ##: Optional counter
                                                      #: optional_counter = 1
                                                      optional_counter = 5
```

Unset, it stays the `#:` lines it was written as, at the point in the shape the
defaults gave it. Nothing is materialised: the block says what the option may
hold, not what it holds by default. This is the one place the merge does not
write a default key out as a live value, and it is why an optional key needs
its default spelled out in the block, there being no value to take one from.

An optional key keeps its place in the order (section 6): it is written where
the defaults wrote it, above the declared key its block sat above, or at the
end of its table when nothing followed it.

#### Reading the blocks

A block ends where the next one begins. A blank line ends one, and so does a
sample line opening a `[table]` header: it names a key of its own. Prose
written directly above such a header introduces it, and goes with the block the
header opens.

Each block is read against the section the headers before it opened, the way a
file of its own would be, so a bare assignment after a header lands inside it:

```toml
#: [[accounts]]
#: name = "Personal"

##: Case-insensitive globs to omit from discovery.
#: ignored_folders = ["Spam"]        <- accounts.ignored_folders, not a root key
```

A second `[[entry]]` block extends the array, as it would in a file of its own:
it is another example of what one entry may hold. The person's first entry is
the one documented, and every example past it stays written out, like any
optional key nobody has set.

A block is left where it was written, merging into nothing, when it is not
TOML, when it names a key another block has already named, or when it ends in
prose that introduces no key. The second covers a defaults author showing two
ways to fill in one `[table]`, which means both to be read; TOML has no way to
say "either of these", so neither can the template. The third follows from
prose introducing what comes after it: prose with nothing after it introduces
nothing, and reading the block would drop it. A note belongs above the key it
is about.

A path reaches through an array of tables, whose entries share a shape, so
`accounts.outgoing` names the `outgoing` table of an `[[accounts]]` entry. The
template describes one entry, and documents the first, which is where a person
reads what the rest may hold.

A sample line directly above a key, with no blank line between, is that key's
recorded default under section 4.2 and not a block of its own. A sample for a
different option belongs a blank line away.

### 4.5 Text after the last key

Marker lines the defaults put after their last key belong to no key. They reach
the user's file all the same, at the end of it, above whatever the person wrote
there. A block among them that anchors a key the person declares travels to
that key like any other.

## 5. Merge rules

Both documents are walked together. The defaults provide the shape and the
order; the user provides the values.

| Situation | Result | Report |
|---|---|---|
| Key in both, same type, leaf | user's value and formatting, plus doc block | nothing |
| Float default, integer user value | user's value and formatting, plus doc block | nothing |
| Key in both, both tables | recurse | nothing |
| Key in both, incompatible types | user's item preserved unchanged | `TypeMismatch`, error |
| Key only in defaults | default's key and value inserted, with docs | nothing |
| Key only in user file | user's key preserved unchanged, appended | `UnknownKey`, warning |
| Key documented but not declared, set by user | merged as any other key, `#:` line recorded above | nothing |
| Key documented but not declared, unset | left as the `#:` lines it was written as | nothing |

### 5.1 Arrays

An array in the user's file replaces the default array entirely. Elements are
not unioned, appended or deduplicated.

```toml
# defaults          # user              # merged
hosts = ["a", "b"]  hosts = ["c"]       hosts = ["c"]
```

### 5.2 Arrays of tables

Same rule. If the user declares any `[[server]]`, that set is complete and the
defaults contribute nothing, including to entries that look like they correspond.

```toml
# defaults        # user            # merged
[[server]]        [[server]]        [[server]]
name = "primary"  name = "mine"     name = "mine"
port = 8080
```

Matching entries by an identity field is deliberately not done. It requires the
application to nominate an identity key per array, produces surprising partial
entries, and has no obvious answer when an entry matches nothing.

### 5.3 Unknown keys are kept

A key the defaults do not declare is never deleted. It may be a typo, an option
from a newer version of the application, or something the person added on
purpose. Deleting a person's text is not a decision this crate makes. It is
reported so the application can warn.

### 5.4 Type mismatches are kept and reported

A user's `timeout = "fast"` where the defaults declare an integer stays in the
file exactly as written, and produces an error-level diagnostic with the line
and column. Overwriting it would destroy the person's input at the moment they
most need to see it. The application decides whether to refuse to start.

## 6. Ordering

The defaults are read first, so the merged document follows the default
document's declaration order. Keys the defaults do not declare come after, in
the order the user wrote them.

```toml
# defaults    # user      # merged
a = 1         b = 2       a = 1
                          b = 2
```

Two constraints follow from TOML's own grammar and are handled during emission:

- root-level key-value pairs must precede the first `[table]` header, so a
  user-only root key is appended to the leaf region, not after the tables;
- standalone tables render according to their recorded position, which is
  renumbered in emission order.

## 7. Diagnostics

Each diagnostic carries a dotted path, a 1-based line and column into the user
source, a severity, and a kind:

- `UnknownKey` (warning): the defaults do not declare this key.
- `TypeMismatch { expected, found }` (error): the user's value has an incompatible
  TOML type. Integers are accepted for float defaults; floats remain invalid for
  integer defaults.
- `Migrated { from }` (warning): a rename rule moved this value.

Positions come from the user document's spans, which is why they are collected
before the document is mutated (see section 10).

## 8. Migrations

A rename map moves a value from an old dotted path to a new one:

```rust
MergeOptions::new()
    .migrate("server.timeout", "network.timeout_seconds")
```

The rule applies when the old path is present in the user document and the new
path is absent. When both are present the user's explicit new-path value wins
and the old one falls through to the unknown-key rule. Migrations run before
the merge, so a migrated value takes part in the merge as if the person had
written it at the new path.

## 9. Public API

```rust
pub fn merge(default_src: &str, user_src: &str) -> Result<Merged, Error>;

pub struct MergeOptions { /* markers, migrations */ }

impl MergeOptions {
    pub fn new() -> Self;
    pub fn markers(self, prose: impl Into<String>, sample: impl Into<String>) -> Self;
    pub fn migrate(self, from: &str, to: &str) -> Self;
    pub fn merge(&self, default_src: &str, user_src: &str) -> Result<Merged, Error>;
}

pub struct Merged { pub report: Report, /* document, newline */ }

impl Merged {
    pub fn document(&self) -> &toml_edit::DocumentMut;
    pub fn to_toml_string(&self) -> String;
    pub fn newline(&self) -> Newline;
}

pub enum Newline { Lf, CrLf }

pub struct Report { /* diagnostics */ }

impl Report {
    pub fn diagnostics(&self) -> &[Diagnostic];
    pub fn has_errors(&self) -> bool;
}

pub struct Diagnostic {
    pub kind: DiagnosticKind,
    pub severity: Severity,
    pub path: String,
    pub line: usize,
    pub column: usize,
}

pub enum Error {
    DefaultParse { source: toml_edit::TomlError },
    UserParse { source: toml_edit::TomlError },
    DefaultsDeclareNoKeys,
    MigrationPath { source: toml_edit::TomlError },
}
```

Which document failed to parse is part of the error, because a broken default
document is the application's fault and a broken user document is the person's.
`DefaultsDeclareNoKeys` is the defaults parsing into text that declares nothing
and documents nothing, which is an application bug; a defaults document of
marker lines alone is not this, because an anchored block documents an option
without declaring it, and a zero-byte defaults document is a separate case and
is allowed. `migrate` takes its paths as written and reads
them as TOML key paths at merge time, so an unreadable one fails the merge,
not the builder call.

`to_toml_string()` produces the file to write back. `document()` is the same
content for callers that want to deserialize it. There is one document because
every default key is materialised with a live value, so the file on disk and the
effective configuration cannot drift apart.

## 10. Implementation notes

Built on `toml_edit` 0.25.

**Spans are lost on mutation.** `toml_edit::Document::parse` keeps byte spans;
`Document::into_mut()` resolves them into owned strings and returns an editable
`DocumentMut` whose `span()` accessors return `None`. Diagnostic positions must
therefore be collected from the user document before it is converted. `despan`
is private to `toml_edit`, so there is no way back.

**Comment text lives in `Decor` prefixes.** For a key-value pair the prefix is on
`Key::leaf_decor()`; for a standalone table it is on `Table::decor()`. A prefix
is one raw string covering everything between the previous item and this one:
blank lines, indentation and comment lines together. Classifying that string
line by line, and rebuilding it per section 4.3, is the whole of the comment
machinery.

**Value comparison** for "does the user's value differ from the default" uses
the rendered representation with decor stripped, not `PartialEq`. This avoids
float equality and treats `1_000` and `1000` as the different text they are.

**Transplanting** a user value moves the user's `Key` and `Item` into the output
document, so the value's own formatting (quoting style, inline table spacing,
multi-line array layout) comes along without being re-rendered.

**Line endings are the person's.** The comment machinery works in lines and
writes them back joined with `\n`, so a file written with `\r\n` would come
back rewritten from top to bottom, every line of it, the person's own comments
included. The ending is read off the user document, a single `\r\n` making it a
CRLF file, and `to_toml_string()` writes every line with it. The defaults decide
nothing here: the file being written back is the person's. `document()` holds
the merged content with `\n`, which is what a caller deserializing it wants.

## 11. Corpus

Merge behaviour is specified by text files in `corpus/`, named `NNNN.txt`. They
are the primary specification: a change in merge behaviour is a change to the
corpus.

```
--- DEF ---
#: Normal counter
counter = 0
--- USR ---
counter = 1
--- RES ---
#: Normal counter
#: counter = 0
counter = 1
```

Grammar:

- `--- DEF ---` opens a default document and starts a group.
- `--- USR ---` opens a user document, `--- RES ---` the expected output.
- One `DEF` may be followed by several `USR`/`RES` pairs, each a case against
  that same default.
- A file may hold several `DEF` groups.
- A line whose content is exactly a `###` comment is a note for the reader,
  stripped before parsing, allowed anywhere. `##` and `####` are not corpus
  comments and pass through as TOML text.
- A section with no content is an empty document.

`tests/corpus.rs` walks the files in order and compares each merge against its
`RES` section exactly, including the trailing newline. It then merges every
`RES` section against its own `DEF` again and requires the text not to move,
which puts section 13's idempotence under test.

Diagnostics, migrations, options and the CLI are covered by ordinary Rust tests.
The corpus is for merge output only.

## 12. Command line

```
documented-toml merge --default D.toml --user U.toml [--in-place | --output OUT]
documented-toml check --default D.toml --user U.toml
```

`merge` writes the merged document and prints diagnostics to stderr. `check`
writes nothing. Both exit non-zero when the report contains an error. `--in-place`
writes a sibling temporary file and renames it over the target, so an
interrupted run cannot leave a truncated config behind.

## 13. Deliberate limits

- No element-wise merging of arrays or arrays of tables.
- No schema, no validation beyond TOML type agreement with the defaults.
- No preservation of a person's edits to marker lines.
- An optional key's default has to be written into its `#:` line by hand. There
  is no value to take one from, which is what makes it optional.
- Two blocks naming the same `[table]` are not merged. The second is left where
  it was written, so an alternative way of filling one table reads as a note
  beside the option and not as part of it.
- A TOML comment beginning with `###` cannot appear inside a corpus file.
- The merge is not idempotent across a change in the defaults, which is the
  point; it is idempotent when the defaults are unchanged.

## 14. Open

The order within a comment block, marker lines above and user lines directly
above the key (section 4.3), is a choice. No example so far requires it. Corpus
file 0004 encodes it and is the single file to change if the opposite order is
wanted.

Whether an optional key set to exactly its documented default should still have
that default recorded above it is open. It does not today, following section
4.2, so the `#:` line disappears at the moment the person's value matches it.

Where the documentation goes when a person has several `[[accounts]]` entries
is open. It goes on the first, so an 80-line block is written once, not per
entry. Corpus file 0017 has one entry and does not decide it.
