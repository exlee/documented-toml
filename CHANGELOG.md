# Changelog

## [Unreleased]

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
