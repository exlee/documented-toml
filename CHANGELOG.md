# Changelog

## [Unreleased]

## [0.3.0] - 2026-09-26

### Changed

- A section whose keys are all commented keeps its `[table]` header live, so
  setting one of its keys is uncommenting one line rather than uncommenting
  the header too. An array of tables keeps its header commented: an empty
  `[[entry]]` is one entry holding nothing, not the absence of entries.
- `Merged::user_set` answers `false` for an empty table, which is what such a
  header leaves in the document.

### Fixed

- Section spacing looked at the top of a header's comment block rather than
  the line above the header, so a blank line could land above text belonging
  to the section before it.

## [0.2.0] - 2026-09-26

### Changed

- A default the person has not set is written as a `#:` line instead of a
  live key, so the merged document holds their choices and nothing else.
  `MergeOptions::defaults_commented(false)` and `--live-defaults` restore the
  previous behaviour.
- Waiting `#:` lines are separated from the key above them only where the
  defaults left a blank line there, so keys written against each other stay
  against each other.

### Added

- `Merged::user_set(path)`, answering whether the person set a value at a
  dotted path.

## [0.1.5] - 2026-09-15

### Changed

- A comment or blank line ends an alignment group. Keys separated by them no
  longer share a column, and a key alone in its group gets one space.

## [0.1.4] - 2026-09-15

### Fixed

- Keep the documented-but-unset keys of an array of tables' first entry inside
  that entry, above the next entry's header, instead of below the last entry.

## [0.1.3] - 2026-09-10

### Added

- Align the `=` of every key in a section. Comments and blank lines do not end
  the span; values written over several lines are left as they are.
  `MergeOptions::align_values(false)` and `--no-align` keep the spacing as
  written.

## [0.1.2] - 2026-09-08

### Fixed

- Separate sections and their documentation with a blank line, including nested
  sections and entries in arrays of tables.
- Preserve default blank lines when the user supplies none above a key, and
  retain user comment paragraph breaks and indentation.
## [0.1.1] - 2026-09-05

### Fixed

- Accept integer overrides for float defaults while preserving their formatting.
  Float overrides for integer defaults still report a type mismatch.
- Correct the license link in generated crate documentation.

## [0.1.0] - 2026-09-04

### Added

- Merge user TOML with documented defaults while preserving user values,
  comments, formatting, and line endings.
- Support optional defaults, configurable documentation markers, key migrations,
  and diagnostics with source positions.
- Provide a library API and CLI commands for merging and checking configuration.
