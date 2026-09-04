# documented-toml

Format-preserving merge of a person's TOML configuration with the documented
defaults an application ships.

An application ships a defaults file where every option is explained in a
comment above it. A person edits their copy: sets values, writes notes of their
own, moves sections around. The next release adds three options and rewords two
explanations. `documented-toml` produces the file that person should have now.
It holds their values, their comments and their formatting, together with every
option the defaults currently declare.

## Install

```console
$ cargo add documented-toml      # the library
$ cargo install documented-toml  # the command
```

## The comment convention

Three kinds of comment live in the same file and are told apart by their prefix:

```toml
##: Request timeout in seconds.
##: Values above 300 are clamped.
#: timeout = 30
# bumped for ticket 44
timeout = 120
```

`##:` is prose the application owns. `#:` is TOML text the application owns:
either the record of the shipped default, or an example of an option that has
no value yet. Both are rewritten from the defaults on every merge, and removed
when their option leaves the defaults. A `#` on its own belongs to the person
and is carried through. Both prefixes are configurable.

## Example

The defaults an application ships:

```toml
##: How long to wait for the server, in seconds.
timeout = 30

##: Where the log is written.
##: An empty path sends it to stderr.
log = ""

##: How many entries the cache holds.
##: Set it only if the default is too small for you.
#: cache_size = 4096

[ui]
##: Colour scheme.
theme = "dark"
```

The file on the person's disk, written against an older release:

```toml
# bumped for the slow staging box
timeout = 120

[ui]
theme = "light"
```

The merge of the two:

```toml
##: How long to wait for the server, in seconds.
#: timeout = 30
# bumped for the slow staging box
timeout = 120

##: Where the log is written.
##: An empty path sends it to stderr.
log = ""

##: How many entries the cache holds.
##: Set it only if the default is too small for you.
#: cache_size = 4096

[ui]
##: Colour scheme.
#: theme = "dark"
theme = "light"
```

The person's value, their comment and their ordering survived. `log` arrived
from the defaults with the value the defaults give it. `cache_size` has no
value to arrive with, so it arrives as the example that documents it. Where the
person overrode a default, the shipped value is recorded above their line, so
they can see what they changed and put it back.

## Library

```rust
# let default_src = "##: How many.\ncount = 1\n";
# let user_src = "count = 7\n";
let merged = documented_toml::merge(default_src, user_src)?;
let text = merged.to_toml_string();
# assert_eq!(text, "##: How many.\n#: count = 1\ncount = 7\n");
# Ok::<(), documented_toml::Error>(())
```

The configurable form takes the two markers and any rename rules:

```rust
use documented_toml::MergeOptions;

# let default_src = "##: How long.\n[network]\ntimeout = 30\n";
# let user_src = "[server]\ntimeout = 90\n";
let merged = MergeOptions::new()
    .markers("#|", "#=")
    .migrate("server.timeout", "network.timeout")
    .merge(default_src, user_src)?;
# assert!(merged.to_toml_string().contains("timeout = 90"));
# Ok::<(), documented_toml::Error>(())
```

A rename rule fires when the old path is present in the person's file and the
new one is absent. The value moves with the comments the person wrote above it.

### The report

Every merge returns the document and a report of what the person's file holds
that the defaults do not account for:

```rust
use documented_toml::{DiagnosticKind, Severity};

# let default_src = "##: How many.\ncount = 1\n";
# let user_src = "count = 7\nverbose = true\n";
let merged = documented_toml::merge(default_src, user_src)?;
for diagnostic in merged.report.diagnostics() {
    println!(
        "{}:{}: {}: {}: {}",
        diagnostic.line, diagnostic.column,
        diagnostic.severity, diagnostic.path, diagnostic.kind,
    );
}
if merged.report.has_errors() {
    // The document is still usable; the application decides whether to start.
}
# assert_eq!(merged.report.diagnostics().len(), 1);
# assert_eq!(merged.report.diagnostics()[0].kind, DiagnosticKind::UnknownKey);
# assert_eq!(merged.report.diagnostics()[0].severity, Severity::Warning);
# Ok::<(), documented_toml::Error>(())
```

`UnknownKey` is a warning and the value is kept. `TypeMismatch` is an error and
the value is kept as written. `Migrated` is a warning naming where the value
came from.

### The merged document is the configuration

Every key the defaults declare is written into the output with a live value, so
there is no second layering pass to run. `merged.document()` borrows the
`toml_edit::DocumentMut` and `merged.into_document()` takes it, which is what a
caller deserializing into its own types wants.

### Line endings

A file written with `\r\n` comes back with `\r\n`, the lines the merge adds
included. The defaults decide nothing here, because the file being written back
is the person's. `merged.newline()` reports which ending was used.

## Command line

```console
$ documented-toml merge --default D.toml --user U.toml [--in-place | --output OUT]
$ documented-toml check --default D.toml --user U.toml
```

`merge` writes the merged document, to stdout unless `--output` or `--in-place`
is given. `check` writes nothing. Both print diagnostics to stderr and exit
non-zero when the report holds an error:

```console
$ documented-toml check --default D.toml --user U.toml
U.toml:3:1: warning: verbose: no such option in the defaults
```

`--in-place` writes a sibling temporary file and renames it over the target, so
an interrupted run cannot leave a truncated config behind.

## Limits

- Arrays and arrays of tables are taken whole from the person's file. There is
  no element-wise merge.
- There is no schema and no validation beyond TOML type agreement with the
  defaults.
- A person's edits to `##:` and `#:` lines are replaced on the next merge.
- An optional key's default is written into its `#:` line by hand, since there
  is no value to take one from.

## Specification

`doc/design.md` states the merge rules. `corpus/` holds the output they produce,
as text files that `tests/corpus.rs` walks and compares byte for byte; it also
re-merges every expected output against its own defaults and requires the text
not to move. A change in merge behaviour is a change to the corpus.

## License

X11. See [`LICENSE`](LICENSE).
