//! The corpus harness: types only, for now.
//!
//! Merge behaviour is specified by the text files in `corpus/`, which are the
//! primary specification. A change in merge behaviour is a change to the
//! corpus. These are the types the harness parses those files into, drawn from
//! the corpus package of `doc/model/model.yml`.
//!
//! The parser, the runner and the cases themselves arrive with the merge.

// Nothing calls these until the harness lands.
#![allow(dead_code)]

/// One `corpus/NNNN.txt` file.
struct CorpusFile {
    /// Path of the file, for failure messages.
    path: String,
    /// The groups in the file, in the order they appear.
    groups: Vec<Group>,
    /// Notes for the reader, stripped before the sections are parsed.
    comments: Vec<CorpusComment>,
}

/// One default document and every case stated against it.
///
/// A `--- DEF ---` section opens a group; the `--- USR ---` and `--- RES ---`
/// pairs that follow all run against the same default.
struct Group {
    /// The text of the `--- DEF ---` section.
    default_src: String,
    /// The cases stated against this default.
    cases: Vec<Case>,
}

/// One user document and its expected merge output.
struct Case {
    /// Position within the file, counting from 1, for failure messages.
    index: usize,
    /// The text of the `--- USR ---` section.
    user_src: String,
    /// The text of the `--- RES ---` section, compared exactly, including the
    /// trailing newline.
    expected: String,
}

/// A delimiter line and the text it opens.
struct Section {
    /// Which delimiter opened it.
    kind: SectionKind,
    /// 1-based line of the delimiter, for failure messages.
    line: usize,
    /// Everything between this delimiter and the next one.
    text: String,
}

/// The three delimiters.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum SectionKind {
    /// `--- DEF ---`, opening a default document and a group.
    Def,
    /// `--- USR ---`, opening a user document.
    Usr,
    /// `--- RES ---`, opening the expected output.
    Res,
}

/// A note for the reader.
///
/// Exactly three hashes, stripped before parsing, allowed anywhere in the file.
/// `##` and `####` are not corpus comments and pass through as TOML text.
struct CorpusComment {
    /// The line as it appeared.
    text: String,
    /// 1-based line it was stripped from.
    line: usize,
}

/// Runs every corpus file and reports the failures.
struct CorpusHarness;

/// What running one case produced.
struct CaseOutcome {
    /// Whether the merge output matched the `--- RES ---` section exactly.
    passed: bool,
    /// What the merge produced, for the diff on failure.
    actual: String,
}
