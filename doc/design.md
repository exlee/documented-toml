# toml-merge design

## 1. Purpose

`toml-merge` reconciles two TOML documents:

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
here. `toml-merge` adds the merge policy, the documentation-comment convention,
and the report.

## 3. Vocabulary

**Default document** the shipped configuration, parsed from the application's
own source. Authored by the application's maintainers.

**User document** the file on the person's disk.

**Merged document** the output. It is both the file to write back and the
effective configuration, because every default key is materialised into it with
a live value. A caller that needs typed access deserializes the merged document
and needs no second layering pass.

**Marker** the comment prefix that identifies documentation owned by the tool.
`#:` by default, configurable.

**Doc block** the run of marker lines that sits immediately above a key.

## 4. The comment convention

Borrowed from kitty's `kitty.conf`. Two kinds of comment coexist in a user's
file and are told apart by their prefix:

```toml
#: Request timeout in seconds.
#: Values above 300 are clamped.
# bumped for ticket 44
timeout = 120
```

Lines starting with the marker (`#:`) belong to `toml-merge`. They are rewritten
from the defaults on every merge and removed when their key leaves the defaults.
Anything a person writes there is lost on the next merge, which is the price of
having documentation that stays current.

Lines starting with a plain `#` belong to the person. The merge never rewrites,
reflows or reorders them. They travel with their key.

A line is a marker line when its first non-whitespace characters are exactly the
marker and the run of `#` is not longer than the marker's. `#:` is a marker
line; `##:` and `#` are not.

### 4.1 In the default document

Only marker lines propagate. A plain `#` comment in the defaults is a note for
whoever maintains the defaults and never reaches a user's file.

```toml
# TODO: revisit before 1.0        <- stays in the defaults
#: Request timeout in seconds.    <- reaches the user's file
timeout = 30
```

### 4.2 Recording the shipped default

When the user's value differs from the shipped default, the merge records the
default above the key as a marker line:

```toml
#: Request timeout in seconds.
#: timeout = 30
timeout = 120
```

The person can see what they departed from without opening the application's
source. When the value equals the default, or the key was absent and the merge
inserted it, no such line is written: the live value already is the default.

### 4.3 Block assembly

The text above a key is rebuilt in this order:

1. blank lines the user had above the block, preserved;
2. the marker documentation lines from the current defaults;
3. the marker line recording the shipped default, when the value was overridden;
4. the user's own `#` lines, in their original order;
5. the key.

The user's notes sit closest to the key they annotate; the refreshable block
sits above them.

## 5. Merge rules

Both documents are walked together. The defaults provide the shape and the
order; the user provides the values.

| Situation | Result | Report |
|---|---|---|
| Key in both, same type, leaf | user's value and formatting, plus doc block | nothing |
| Key in both, both tables | recurse | nothing |
| Key in both, types differ | user's item preserved unchanged | `TypeMismatch`, error |
| Key only in defaults | default's key and value inserted, with docs | nothing |
| Key only in user file | user's key preserved unchanged, appended | `UnknownKey`, warning |

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
- `TypeMismatch { expected, found }` (error): the user's value has a different
  TOML type from the default's.
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

pub struct MergeOptions { /* marker, migrations */ }

impl MergeOptions {
    pub fn new() -> Self;
    pub fn marker(self, marker: impl Into<String>) -> Self;
    pub fn migrate(self, from: &str, to: &str) -> Self;
    pub fn merge(&self, default_src: &str, user_src: &str) -> Result<Merged, Error>;
}

pub struct Merged { pub report: Report, /* document */ }

impl Merged {
    pub fn document(&self) -> &toml_edit::DocumentMut;
    pub fn to_toml_string(&self) -> String;
}

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
to merge, which is an application bug; a zero-byte defaults document is a
separate case and is allowed. `migrate` takes its paths as written and reads
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
toml-merge merge --default D.toml --user U.toml [--in-place | --output OUT]
toml-merge check --default D.toml --user U.toml
```

`merge` writes the merged document and prints diagnostics to stderr. `check`
writes nothing. Both exit non-zero when the report contains an error. `--in-place`
writes a sibling temporary file and renames it over the target, so an
interrupted run cannot leave a truncated config behind.

## 13. Deliberate limits

- No element-wise merging of arrays or arrays of tables.
- No schema, no validation beyond TOML type agreement with the defaults.
- No preservation of a person's edits to marker lines.
- A TOML comment beginning with `###` cannot appear inside a corpus file.
- The merge is not idempotent across a change in the defaults, which is the
  point; it is idempotent when the defaults are unchanged.

## 14. Open

The order within a comment block, marker lines above and user lines directly
above the key (section 4.3), is a choice. No example so far requires it. Corpus
file 0004 encodes it and is the single file to change if the opposite order is
wanted.
