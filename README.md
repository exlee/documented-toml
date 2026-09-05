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
comments remain. Both markers are configurable.

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
log = ""

##: Maximum cache entries.
#: cache_size = 4096
```

New settings come from the defaults. Overridden defaults become `#:` lines, so
the shipped value remains visible. Optional settings stay commented.

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

Each merge includes a [`Report`](https://docs.rs/documented-toml/latest/documented_toml/struct.Report.html):

- `UnknownKey`: warning; the value remains.
- `TypeMismatch`: error; the value remains as written.
- `Migrated`: warning naming the old path.

Use `merged.document()` to borrow the resulting `toml_edit::DocumentMut`, or
`merged.into_document()` to take it for deserialization. `merged.newline()`
reports the selected line ending.

## Command line

```console
documented-toml merge --default D.toml --user U.toml [--in-place | --output OUT]
documented-toml check --default D.toml --user U.toml
```

`merge` writes to stdout unless given `--output` or `--in-place`. `check` writes
no document. Both commands print diagnostics to stderr and return a non-zero
status for errors.

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
