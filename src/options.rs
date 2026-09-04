//! What the caller configures before a merge.

use toml_edit::TomlError;

use crate::decor::Marker;
use crate::error::Error;
use crate::merge::{MergeEngine, Merged};
use crate::path::DottedPath;
use crate::source::SourceDocument;

/// Builder holding the marker and the rename rules.
#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct MergeOptions {
    pub(crate) marker: Marker,
    pub(crate) migrations: Vec<Migration>,
}

impl MergeOptions {
    /// The default marker and no rename rules.
    pub fn new() -> Self {
        Self::default()
    }

    /// Sets the two comment prefixes the tool owns: prose first, then the one
    /// on its TOML text.
    pub fn markers(mut self, prose: impl Into<String>, sample: impl Into<String>) -> Self {
        self.marker = Marker::new(prose, sample);
        self
    }

    /// Adds a rename rule, moving a value from one dotted path to another.
    ///
    /// The paths are read as TOML key paths, so a quoted segment holding a dot
    /// is one segment. An unparsable path is an error at merge time, not here.
    pub fn migrate(mut self, from: &str, to: &str) -> Self {
        self.migrations.push(Migration {
            from: DottedPath::parse(from),
            to: DottedPath::parse(to),
        });
        self
    }

    /// The prefix this merge writes prose under.
    pub fn prose_marker(&self) -> &str {
        self.marker.prose()
    }

    /// The prefix this merge writes TOML text under.
    pub fn sample_marker(&self) -> &str {
        self.marker.sample()
    }

    /// Merges a user document against the defaults.
    pub fn merge(&self, default_src: &str, user_src: &str) -> Result<Merged, Error> {
        let defaults =
            SourceDocument::parse(default_src).map_err(|source| Error::DefaultParse { source })?;
        if defaults.has_text_but_no_keys()
            && !default_src.lines().any(|line| self.marker.owns(line))
        {
            return Err(Error::DefaultsDeclareNoKeys);
        }
        let (user, spans) = SourceDocument::parse_with_spans(user_src)
            .map_err(|source| Error::UserParse { source })?;
        let migrations = self.resolved_migrations()?;
        Ok(MergeEngine::new(defaults, user, user_src, self.clone(), migrations, spans).run())
    }

    fn resolved_migrations(&self) -> Result<Vec<ResolvedMigration>, Error> {
        self.migrations
            .iter()
            .map(|migration| {
                Ok(ResolvedMigration {
                    from: migration
                        .from
                        .clone()
                        .map_err(|source| Error::MigrationPath { source })?,
                    to: migration
                        .to
                        .clone()
                        .map_err(|source| Error::MigrationPath { source })?,
                })
            })
            .collect()
    }
}

/// One rename rule, applied before the merge walk.
///
/// It fires when `from` is present in the user document and `to` is absent.
/// The paths are held as written until the merge reads them, so a builder call
/// never fails.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Migration {
    pub(crate) from: Result<DottedPath, TomlError>,
    pub(crate) to: Result<DottedPath, TomlError>,
}

/// A rename rule whose paths parsed.
#[derive(Debug, Clone, PartialEq, Eq)]
pub(crate) struct ResolvedMigration {
    pub(crate) from: DottedPath,
    pub(crate) to: DottedPath,
}
