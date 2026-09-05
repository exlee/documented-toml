# Changelog

## [Unreleased]

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
