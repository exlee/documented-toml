//! The corpus harness.
//!
//! Merge behaviour is specified by the text files in `corpus/`, which are the
//! primary specification. A change in merge behaviour is a change to the
//! corpus.

use std::fs;
use std::path::Path;

use documented_toml::MergeOptions;
use nonempty::NonEmpty;

/// One `corpus/NNNN.txt` file.
#[derive(Debug)]
struct CorpusFile {
    /// Path of the file, for failure messages.
    path: String,
    /// The groups in the file, in the order they appear. A file with no
    /// `--- DEF ---` section is malformed, not empty.
    groups: NonEmpty<Group>,
    /// Notes for the reader, stripped before the sections are parsed.
    comments: Vec<CorpusComment>,
}

/// One default document and every case stated against it.
///
/// A `--- DEF ---` section opens a group; the `--- USR ---` and `--- RES ---`
/// pairs that follow all run against the same default. Flags after the
/// delimiter set merge options for the whole group: `align` lines up `=`.
#[derive(Debug)]
struct Group {
    /// The text of the `--- DEF ---` section.
    default_src: String,
    /// Whether the group merges with `=` aligned.
    align: bool,
    /// The cases stated against this default. A `--- DEF ---` with no
    /// `--- USR ---` after it is malformed, not empty.
    cases: NonEmpty<Case>,
}

/// One user document and its expected merge output.
#[derive(Debug)]
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
#[derive(Debug)]
struct Section {
    /// Which delimiter opened it.
    kind: SectionKind,
    /// The words after the delimiter.
    flags: Vec<String>,
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
#[derive(Debug)]
struct CorpusComment {
    /// The line as it appeared.
    text: String,
    /// 1-based line it was stripped from.
    line: usize,
}

/// Runs every corpus file and reports the failures.
struct CorpusHarness;

/// What running one case produced.
#[derive(Debug)]
struct CaseOutcome {
    /// Whether the merge output matched the `--- RES ---` section exactly.
    passed: bool,
    /// What the merge produced, for the diff on failure.
    actual: String,
}

// -- parsing ------------------------------------------------------------

/// The delimiter opening each kind of section.
fn delimiter(kind: SectionKind) -> &'static str {
    match kind {
        SectionKind::Def => "--- DEF ---",
        SectionKind::Usr => "--- USR ---",
        SectionKind::Res => "--- RES ---",
    }
}

/// The delimiter a line opens, with the flags written after it.
fn delimiter_kind(line: &str) -> Option<(SectionKind, Vec<String>)> {
    [SectionKind::Def, SectionKind::Usr, SectionKind::Res]
        .into_iter()
        .find_map(|kind| {
            let rest = line.strip_prefix(delimiter(kind))?;
            if !rest.is_empty() && !rest.starts_with(' ') {
                return None;
            }
            Some((kind, rest.split_whitespace().map(str::to_owned).collect()))
        })
}

/// Whether a line is a note for the reader.
///
/// Exactly three hashes: `##` and `####` are TOML text and pass through.
fn is_corpus_comment(line: &str) -> bool {
    let text = line.trim_start();
    text.starts_with("###") && !text.starts_with("####")
}

impl CorpusFile {
    /// Reads one corpus file. Every way of being malformed is an `Err`, because
    /// a corpus file that does not state a case states nothing.
    fn parse(path: &Path, text: &str) -> Result<Self, String> {
        let display = path.display().to_string();
        let mut comments = Vec::new();
        let mut sections: Vec<Section> = Vec::new();

        for (offset, line) in text.lines().enumerate() {
            let number = offset + 1;
            if is_corpus_comment(line) {
                comments.push(CorpusComment {
                    text: line.to_owned(),
                    line: number,
                });
                continue;
            }
            match delimiter_kind(line) {
                Some((kind, flags)) => sections.push(Section {
                    kind,
                    flags,
                    line: number,
                    text: String::new(),
                }),
                None => match sections.last_mut() {
                    Some(section) => {
                        section.text.push_str(line);
                        section.text.push('\n');
                    }
                    None if line.trim().is_empty() => {}
                    None => {
                        return Err(format!(
                            "{display}:{number}: text before the first delimiter"
                        ));
                    }
                },
            }
        }

        let groups = Self::group(&display, sections)?;
        let groups = NonEmpty::from_vec(groups)
            .ok_or_else(|| format!("{display}: no --- DEF --- section"))?;
        Ok(Self {
            path: display,
            groups,
            comments,
        })
    }

    fn group(display: &str, sections: Vec<Section>) -> Result<Vec<Group>, String> {
        let mut collected: Vec<(String, bool, Vec<Case>)> = Vec::new();
        let mut pending: Option<(usize, String)> = None;
        let mut index = 0;

        for section in sections {
            match section.kind {
                SectionKind::Def => {
                    if let Some((line, _)) = pending {
                        return Err(format!("{display}:{line}: --- USR --- with no --- RES ---"));
                    }
                    let mut align = false;
                    for flag in &section.flags {
                        match flag.as_str() {
                            "align" => align = true,
                            other => {
                                return Err(format!(
                                    "{}:{}: unknown flag {other}",
                                    display, section.line
                                ));
                            }
                        }
                    }
                    collected.push((section.text, align, Vec::new()));
                }
                SectionKind::Usr => {
                    if collected.is_empty() {
                        return Err(format!(
                            "{}:{}: --- USR --- before any --- DEF ---",
                            display, section.line
                        ));
                    }
                    if let Some((line, _)) = pending {
                        return Err(format!("{display}:{line}: --- USR --- with no --- RES ---"));
                    }
                    if !section.flags.is_empty() {
                        return Err(format!(
                            "{}:{}: only --- DEF --- takes flags",
                            display, section.line
                        ));
                    }
                    pending = Some((section.line, section.text));
                }
                SectionKind::Res => {
                    let Some((_, user_src)) = pending.take() else {
                        return Err(format!(
                            "{}:{}: --- RES --- with no --- USR ---",
                            display, section.line
                        ));
                    };
                    if !section.flags.is_empty() {
                        return Err(format!(
                            "{}:{}: only --- DEF --- takes flags",
                            display, section.line
                        ));
                    }
                    index += 1;
                    collected
                        .last_mut()
                        .expect("a group exists once a case is pending")
                        .2
                        .push(Case {
                            index,
                            user_src,
                            expected: section.text,
                        });
                }
            }
        }
        if let Some((line, _)) = pending {
            return Err(format!("{display}:{line}: --- USR --- with no --- RES ---"));
        }

        collected
            .into_iter()
            .map(|(default_src, align, cases)| {
                let cases = NonEmpty::from_vec(cases)
                    .ok_or_else(|| format!("{display}: a --- DEF --- states no cases"))?;
                Ok(Group {
                    default_src,
                    align,
                    cases,
                })
            })
            .collect()
    }
}

// -- running ------------------------------------------------------------

impl Group {
    fn options(&self) -> MergeOptions {
        MergeOptions::new().align_values(self.align)
    }
}

impl Case {
    fn run(&self, group: &Group) -> CaseOutcome {
        let merged = group
            .options()
            .merge(&group.default_src, &self.user_src)
            .expect("a corpus case parses on both sides");
        let actual = merged.to_toml_string();
        CaseOutcome {
            passed: actual == self.expected,
            actual,
        }
    }

    /// Merging the output again against the same defaults must not move it.
    /// The merge is idempotent while the defaults are unchanged, which is what
    /// makes it safe to run on every start-up.
    fn run_again(&self, group: &Group, once: &str) -> CaseOutcome {
        let merged = group
            .options()
            .merge(&group.default_src, once)
            .expect("merged output parses as TOML again");
        let actual = merged.to_toml_string();
        CaseOutcome {
            passed: actual == once,
            actual,
        }
    }
}

impl CorpusHarness {
    fn files() -> Vec<CorpusFile> {
        let dir = Path::new(env!("CARGO_MANIFEST_DIR")).join("corpus");
        let mut paths: Vec<_> = fs::read_dir(&dir)
            .unwrap_or_else(|e| panic!("corpus directory {}: {e}", dir.display()))
            .map(|entry| entry.expect("a readable directory entry").path())
            .filter(|path| path.extension().is_some_and(|e| e == "txt"))
            .collect();
        paths.sort();
        assert!(!paths.is_empty(), "no corpus files in {}", dir.display());
        paths
            .iter()
            .map(|path| {
                let text = fs::read_to_string(path).expect("a readable corpus file");
                CorpusFile::parse(path, &text).unwrap_or_else(|e| panic!("{e}"))
            })
            .collect()
    }
}

fn report(failures: Vec<String>) {
    if !failures.is_empty() {
        panic!("\n{}", failures.join("\n"));
    }
}

#[test]
fn corpus_states_the_merge() {
    let mut failures = Vec::new();
    for file in CorpusHarness::files() {
        for group in &file.groups {
            for case in &group.cases {
                let outcome = case.run(group);
                if !outcome.passed {
                    failures.push(format!(
                        "{} case {}:\n--- expected ---\n{}--- actual ---\n{}",
                        file.path, case.index, case.expected, outcome.actual
                    ));
                }
            }
        }
    }
    report(failures);
}

#[test]
fn corpus_output_is_a_fixed_point() {
    let mut failures = Vec::new();
    for file in CorpusHarness::files() {
        for group in &file.groups {
            for case in &group.cases {
                let once = case.run(group).actual;
                let outcome = case.run_again(group, &once);
                if !outcome.passed {
                    failures.push(format!(
                        "{} case {} moved on a second merge:\n--- once ---\n{}--- twice ---\n{}",
                        file.path, case.index, once, outcome.actual
                    ));
                }
            }
        }
    }
    report(failures);
}

#[test]
fn a_corpus_file_needs_a_default_section() {
    let error = CorpusFile::parse(Path::new("x.txt"), "--- USR ---\na = 1\n").unwrap_err();
    assert!(error.contains("before any"), "{error}");
}

#[test]
fn a_user_section_needs_a_result_section() {
    let error = CorpusFile::parse(
        Path::new("x.txt"),
        "--- DEF ---\na = 1\n--- USR ---\na = 2\n",
    )
    .unwrap_err();
    assert!(error.contains("no --- RES ---"), "{error}");
}

#[test]
fn corpus_comments_are_stripped_and_lookalikes_are_not() {
    let file = CorpusFile::parse(
        Path::new("x.txt"),
        "### a note\n--- DEF ---\na = 1\n--- USR ---\n## kept\na = 2\n--- RES ---\n#### kept\na = 2\n",
    )
    .unwrap();
    assert_eq!(file.comments.len(), 1);
    assert_eq!(file.comments[0].text, "### a note");
    assert_eq!(file.comments[0].line, 1);
    assert!(file.groups.head.cases.head.user_src.contains("## kept"));
    assert!(file.groups.head.cases.head.expected.contains("#### kept"));
}

#[test]
fn a_default_section_takes_the_align_flag_and_nothing_else() {
    let file = CorpusFile::parse(
        Path::new("x.txt"),
        "--- DEF --- align\na = 1\n--- USR ---\n--- RES ---\na = 1\n",
    )
    .unwrap();
    assert!(file.groups.head.align);
    let plain = CorpusFile::parse(
        Path::new("x.txt"),
        "--- DEF ---\na = 1\n--- USR ---\n--- RES ---\na = 1\n",
    )
    .unwrap();
    assert!(!plain.groups.head.align);
    let error = CorpusFile::parse(
        Path::new("x.txt"),
        "--- DEF --- wat\na = 1\n--- USR ---\n--- RES ---\na = 1\n",
    )
    .unwrap_err();
    assert!(error.contains("unknown flag"), "{error}");
    let error = CorpusFile::parse(
        Path::new("x.txt"),
        "--- DEF ---\na = 1\n--- USR --- align\n--- RES ---\na = 1\n",
    )
    .unwrap_err();
    assert!(error.contains("only --- DEF ---"), "{error}");
    let lookalike = CorpusFile::parse(Path::new("x.txt"), "--- DEF ---x\n").unwrap_err();
    assert!(
        lookalike.contains("before the first delimiter"),
        "{lookalike}"
    );
}

#[test]
fn a_section_with_no_content_is_an_empty_document() {
    let file = CorpusFile::parse(
        Path::new("x.txt"),
        "--- DEF ---\na = 1\n--- USR ---\n--- RES ---\na = 1\n",
    )
    .unwrap();
    assert_eq!(file.groups.head.cases.head.user_src, "");
}
