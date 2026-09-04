//! Classification and reassembly of the text above a key.
//!
//! A `toml_edit::Decor` prefix is one raw string covering everything between
//! the previous item and this key: blank lines, indentation and comment lines
//! together. These types split that string and build the replacement.

use toml_edit::Decor;

/// The two comment prefixes the tool owns.
///
/// `##:` is prose: the sentences explaining an option. `#:` is TOML text: the
/// line recording a shipped default, and the samples a defaults author writes
/// for an option that ships with no live value. Both are rewritten from the
/// defaults on every merge. A plain `#` belongs to the person and is never
/// touched.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Marker {
    pub(crate) prose: String,
    pub(crate) sample: String,
}

/// The prose marker, used when the caller sets none.
pub const DEFAULT_PROSE_MARKER: &str = "##:";

/// The sample marker, used when the caller sets none.
pub const DEFAULT_SAMPLE_MARKER: &str = "#:";

impl Marker {
    /// Markers from their literal text.
    pub fn new(prose: impl Into<String>, sample: impl Into<String>) -> Self {
        Self {
            prose: prose.into(),
            sample: sample.into(),
        }
    }

    /// The prefix on a prose line.
    pub fn prose(&self) -> &str {
        &self.prose
    }

    /// The prefix on a line of TOML text.
    pub fn sample(&self) -> &str {
        &self.sample
    }

    /// What one line of a prefix is.
    ///
    /// A line carries a marker when its first non-whitespace characters are
    /// exactly that marker and its run of `#` is the same length as the
    /// marker's. With the default markers `##:` is prose and `#:` is a sample;
    /// `###:` and `#` are the person's.
    pub fn classify(&self, line: &str) -> PrefixLine {
        let text = line.to_owned();
        if line.trim().is_empty() {
            PrefixLine::Blank { text }
        } else if carries(&self.prose, line) {
            PrefixLine::Prose { text }
        } else if carries(&self.sample, line) {
            PrefixLine::Sample { text }
        } else {
            PrefixLine::User { text }
        }
    }

    /// Whether a line is the tool's, of either kind.
    pub fn owns(&self, line: &str) -> bool {
        matches!(
            self.classify(line),
            PrefixLine::Prose { .. } | PrefixLine::Sample { .. }
        )
    }

    /// A sample line with the marker taken off, keeping what follows it so an
    /// indented sample stays indented.
    pub fn undress(&self, line: &str) -> String {
        let text = line.trim_start();
        let body = text
            .strip_prefix(&self.sample)
            .or_else(|| text.strip_prefix(&self.prose))
            .unwrap_or(text);
        body.strip_prefix(' ').unwrap_or(body).to_owned()
    }

    /// The line recording a shipped default.
    fn echo_line(&self, echo: &DefaultEcho) -> String {
        format!("{} {} = {}", self.sample, echo.key, echo.value)
    }
}

impl Default for Marker {
    fn default() -> Self {
        Self::new(DEFAULT_PROSE_MARKER, DEFAULT_SAMPLE_MARKER)
    }
}

fn carries(marker: &str, line: &str) -> bool {
    let text = line.trim_start();
    text.starts_with(marker) && hashes(text) == hashes(marker)
}

fn hashes(text: &str) -> usize {
    text.chars().take_while(|c| *c == '#').count()
}

/// The raw decor text between the previous item and this key.
#[derive(Debug, Clone, PartialEq, Eq, Default)]
pub struct Prefix {
    pub(crate) raw: String,
}

impl Prefix {
    /// The prefix of a decor, empty when it has none.
    pub fn of(decor: &Decor) -> Self {
        let raw = decor
            .prefix()
            .and_then(|raw| raw.as_str())
            .unwrap_or_default()
            .to_owned();
        Self { raw }
    }

    /// A prefix from text that is a whole region of its own, such as the
    /// trailing text at the end of a document.
    pub fn from_text(text: &str) -> Self {
        Self {
            raw: text.to_owned(),
        }
    }

    /// The complete lines of the prefix, classified.
    ///
    /// The text after the last newline is indentation for the key itself, not a
    /// line of its own, and comes back from [`Prefix::indent`].
    pub fn lines(&self, marker: &Marker) -> Vec<PrefixLine> {
        let mut parts: Vec<&str> = self.raw.split('\n').collect();
        parts.pop();
        parts
            .into_iter()
            .map(|line| marker.classify(line))
            .collect()
    }

    /// The prefix split at the last blank line.
    ///
    /// Everything before it stands on its own, separated from the key by that
    /// blank line, and is carried across as it was written. What comes after is
    /// the block that touches the key, which is the block this merge rebuilds.
    pub fn split(&self, marker: &Marker) -> (Vec<PrefixLine>, Vec<PrefixLine>) {
        let lines = self.lines(marker);
        let touching = lines
            .iter()
            .rposition(|line| matches!(line, PrefixLine::Blank { .. }))
            .map_or(0, |at| at + 1);
        (lines[..touching].to_vec(), lines[touching..].to_vec())
    }

    /// The whitespace between the last newline and the key.
    pub fn indent(&self) -> &str {
        match self.raw.rfind('\n') {
            Some(at) => &self.raw[at + 1..],
            None => &self.raw,
        }
    }
}

/// One classified line of a [`Prefix`].
///
/// The text is kept verbatim, including leading whitespace, so a line the merge
/// does not own can be written back exactly as it was found.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum PrefixLine {
    /// The tool's prose. Rewritten from the defaults on every merge.
    Prose {
        /// The line as it appeared, marker included.
        text: String,
    },
    /// The tool's TOML text: a shipped default or a sample. Rewritten on every
    /// merge, and read as an anchor naming the key it documents.
    Sample {
        /// The line as it appeared, marker included.
        text: String,
    },
    /// Owned by the person. Never rewritten, reflowed or reordered.
    User {
        /// The line as it appeared.
        text: String,
    },
    /// Empty or whitespace only.
    Blank {
        /// The line as it appeared.
        text: String,
    },
}

impl PrefixLine {
    /// The line as it appeared.
    pub fn text(&self) -> &str {
        match self {
            Self::Prose { text }
            | Self::Sample { text }
            | Self::User { text }
            | Self::Blank { text } => text,
        }
    }
}

/// The TOML text under a key's prose: either the shipped default the merge
/// records, or the samples the defaults author wrote for an option that ships
/// with no value of its own.
#[derive(Debug, Clone, PartialEq, Eq, Default)]
pub enum Sample {
    /// The key ships no sample and the person has not departed from anything.
    #[default]
    None,
    /// The shipped default, recorded because the person's value differs.
    Echo(DefaultEcho),
    /// Sample lines from the defaults, carried across as written.
    Lines(Vec<String>),
}

/// One key's rebuilt prefix.
///
/// Rendered in this order: text the defaults kept a blank line away from the
/// key, the blank lines above the block, the prose from the current defaults,
/// the TOML text under it, then the person's own comment lines. The person's
/// notes end up closest to the key they annotate.
#[derive(Debug, Clone, PartialEq, Eq, Default)]
pub struct DocBlock {
    /// Text the defaults separated from the key by a blank line, kept verbatim.
    pub(crate) floating: Vec<String>,
    /// Blank lines above the block, as [`PrefixLine::Blank`] text.
    pub(crate) leading_blanks: Vec<String>,
    /// Prose from the defaults, as [`PrefixLine::Prose`] text.
    pub(crate) prose: Vec<String>,
    /// The TOML text under the prose.
    pub(crate) sample: Sample,
    /// The person's own comments, as [`PrefixLine::User`] text.
    pub(crate) user_lines: Vec<String>,
    /// Whitespace before the key itself, taken from whoever supplied the key.
    pub(crate) indent: String,
}

impl DocBlock {
    /// Takes the leading blank lines, the person's comments and the
    /// indentation from a prefix. The marker lines in it are dropped: they
    /// belong to the tool and are written again from the defaults.
    pub(crate) fn keep_user_text(&mut self, prefix: &Prefix, marker: &Marker) {
        let lines = prefix.lines(marker);
        let mut seen_content = false;
        for line in lines {
            match line {
                PrefixLine::Blank { text } if !seen_content => self.leading_blanks.push(text),
                PrefixLine::Blank { .. } => {}
                PrefixLine::Prose { .. } | PrefixLine::Sample { .. } => seen_content = true,
                PrefixLine::User { text } => {
                    seen_content = true;
                    self.user_lines.push(text);
                }
            }
        }
        self.indent = prefix.indent().to_owned();
    }

    /// Takes the prose and the samples from a prefix in the default document.
    /// Plain comments there are notes for whoever maintains the defaults and
    /// never reach a user's file.
    pub(crate) fn take_docs(&mut self, touching: &[PrefixLine]) {
        for line in touching {
            match line {
                PrefixLine::Prose { text } => self.prose.push(text.clone()),
                PrefixLine::Sample { text } => match &mut self.sample {
                    Sample::Lines(lines) => lines.push(text.clone()),
                    _ => self.sample = Sample::Lines(vec![text.clone()]),
                },
                _ => {}
            }
        }
    }

    /// The prefix text to write back.
    pub(crate) fn render(&self, marker: &Marker) -> String {
        let mut out = String::new();
        for line in &self.floating {
            out.push_str(line);
            out.push('\n');
        }
        if self.floating.is_empty() {
            for line in &self.leading_blanks {
                out.push_str(line);
                out.push('\n');
            }
        }
        for line in &self.prose {
            out.push_str(line);
            out.push('\n');
        }
        match &self.sample {
            Sample::None => {}
            Sample::Echo(echo) => {
                out.push_str(&marker.echo_line(echo));
                out.push('\n');
            }
            Sample::Lines(lines) => {
                for line in lines {
                    out.push_str(line);
                    out.push('\n');
                }
            }
        }
        for line in &self.user_lines {
            out.push_str(line);
            out.push('\n');
        }
        out.push_str(&self.indent);
        out
    }
}

/// The marker line recording the shipped default.
///
/// Written only when the user's value differs from it. A key whose value equals
/// the default, or one the merge inserted, gets no echo: the live value already
/// is the default.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct DefaultEcho {
    pub(crate) key: String,
    pub(crate) value: String,
}
