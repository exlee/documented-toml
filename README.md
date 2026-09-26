# documented-toml

Merge new TOML defaults into a user's configuration while preserving their
values, comments, ordering, formatting, and line endings.

Defaults are named for their chief property: someone will change them.

## Install

```console
cargo add documented-toml      # library
cargo install documented-toml  # command
```

## Comment markers

```toml
##: Application documentation.
#: timeout = 30
# The staging server is slow.
timeout = 120
```

- `##:` marks prose owned by the application.
- `#:` marks TOML owned by the application: a shipped default or an optional
  setting.
- Ordinary `#` comments belong to the user.

Application-owned lines are refreshed from the defaults on every merge. User
comments retain their indentation and paragraph breaks. Both markers are
configurable.

Sections are separated by a blank line before their documentation or header.
Existing blank lines are preserved; when the user supplies none above a key,
the spacing from the defaults is used.

## Example

Defaults:

```toml
##: Request timeout in seconds.
timeout = 30

##: Where logs are written. An empty path means stderr.
log = ""

##: Maximum cache entries.
#: cache_size = 4096
```

User configuration:

```toml
# The staging server is slow.
timeout = 120
```

Result:

```toml
##: Request timeout in seconds.
#: timeout = 30
# The staging server is slow.
timeout = 120

##: Where logs are written. An empty path means stderr.
#: log = ""

##: Maximum cache entries.
#: cache_size = 4096
```

The file holds what the user set. Everything else is a `#:` line, so the
shipped value is visible without the file claiming they chose it. A section
keeps its `[table]` header, so setting one of its keys is uncommenting one
line. Ask
`Merged::user_set("log")` which values are theirs, and take the rest from the
defaults you passed in.

`MergeOptions::defaults_commented(false)` (`--live-defaults`) writes every
default as a live key instead, which makes the merged document the effective
configuration and makes `user_set` tell you nothing.

## Library

```rust
# let default_src = "##: How many.\ncount = 1\n";
# let user_src = "count = 7\n";
let merged = documented_toml::merge(default_src, user_src)?;
let text = merged.to_toml_string();
# assert_eq!(text, "##: How many.\n#: count = 1\ncount = 7\n");
# Ok::<(), documented_toml::Error>(())
```

Custom markers and renamed keys use `MergeOptions`:

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

A migration runs when the old path exists and the new path does not. It moves
the value and its user comments.

The `=` of keys on consecutive lines is lined up. A comment or blank line
starts a new group, and a key alone in its group gets one space. A value written over several lines keeps its own
spacing and sets no width. `align_values(false)` keeps the spacing as the
person wrote it.

Each merge includes a [`Report`](https://docs.rs/documented-toml/latest/documented_toml/struct.Report.html):

- `UnknownKey`: warning; the value remains.
- `TypeMismatch`: error; the value remains as written.
- `Migrated`: warning naming the old path.

Use `merged.document()` to borrow the resulting `toml_edit::DocumentMut`, or
`merged.into_document()` to take it for deserialization. `merged.newline()`
reports the selected line ending.

## Defaults and what the user chose

A default nobody set is a `#:` line, not a key, so the merged document holds
the user's choices and nothing else:

```rust
# let default_src = "count = 1\nlimit = 10\n";
# let user_src = "count = 7\n";
let merged = documented_toml::merge(default_src, user_src)?;
assert!(merged.user_set("count"));
assert!(!merged.user_set("limit"));
# assert_eq!(merged.to_toml_string(), "#: count = 1\ncount = 7\n#: limit = 10\n");
# Ok::<(), documented_toml::Error>(())
```

So the values come from two places: the defaults you passed in, overlaid by the
merged document. Keep the defaults in TOML files of their own if you want them
per platform, compose them, and pass the composed text to both steps.

`set` writes a value into the file where the defaults say it goes, turning a
`#:` line into a live key under it:

```rust
# let default_src = "##: How many.\ncount = 1\nlimit = 10\n";
let mut merged = documented_toml::merge(default_src, "")?;
merged.set("limit", 50)?;
assert_eq!(
    merged.to_toml_string(),
    "##: How many.\n#: count = 1\n#: limit = 10\nlimit = 50\n"
);
# Ok::<(), documented_toml::Error>(())
```

It merges again, so the report afterwards describes the file as it now stands.

## Command line

```console
documented-toml merge --default D.toml --user U.toml [--in-place | --output OUT]
                      [--no-align] [--live-defaults]
documented-toml check --default D.toml --user U.toml
```

`merge` writes to stdout unless given `--output` or `--in-place`. `check` writes
no document. Both commands print diagnostics to stderr and return a non-zero
status for errors. `--no-align` keeps the spacing around `=` as the person
wrote it. `--live-defaults` writes a default nobody set as a key rather than a
`#:` line.

`--in-place` writes and syncs a sibling temporary file before renaming it over
the user file.

## Limits

- Arrays and arrays of tables are replaced whole.
- Validation checks TOML types, accepting integer values for float defaults.
  Float values for integer defaults remain errors. There is no schema language.
- User edits to `##:` and `#:` lines are replaced by the next merge.
- Optional defaults must appear in their `#:` line. Clairvoyance is outside the
  public API.

## Specification

[`doc/design.md`](doc/design.md) defines the merge rules. [`corpus/`](corpus/)
contains byte-for-byte examples and idempotence cases.

## License

X11. See [LICENSE](https://github.com/exlee/documented-toml/blob/master/LICENSE).
