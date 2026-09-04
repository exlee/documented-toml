//! What the merge reports, and where it says it happened.
//!
//! Merge output is specified by `corpus/`; these are the observations that
//! travel alongside it.

use toml_merge::{DiagnosticKind, Severity, TomlType, merge};

fn kinds(default_src: &str, user_src: &str) -> Vec<DiagnosticKind> {
    merge(default_src, user_src)
        .expect("both documents parse")
        .report
        .diagnostics()
        .iter()
        .map(|d| d.kind.clone())
        .collect()
}

#[test]
fn a_key_the_defaults_do_not_declare_is_a_warning() {
    let merged = merge("a = 1\n", "a = 1\nb = 2\n").unwrap();
    let diagnostics = merged.report.diagnostics();
    assert_eq!(diagnostics.len(), 1);
    assert_eq!(diagnostics[0].kind, DiagnosticKind::UnknownKey);
    assert_eq!(diagnostics[0].severity, Severity::Warning);
    assert_eq!(diagnostics[0].path.to_string(), "b");
    assert!(!merged.report.has_errors());
}

#[test]
fn a_value_of_the_wrong_type_is_an_error() {
    let merged = merge("timeout = 30\n", "timeout = \"fast\"\n").unwrap();
    assert!(merged.report.has_errors());
    assert_eq!(
        merged.report.diagnostics()[0].kind,
        DiagnosticKind::TypeMismatch {
            expected: TomlType::Integer,
            found: TomlType::String,
        }
    );
}

#[test]
fn a_table_where_a_value_is_declared_is_a_type_mismatch() {
    assert_eq!(
        kinds("timeout = 30\n", "[timeout]\nseconds = 1\n"),
        [DiagnosticKind::TypeMismatch {
            expected: TomlType::Integer,
            found: TomlType::Table,
        }]
    );
}

#[test]
fn an_inline_table_is_not_a_standalone_table() {
    assert_eq!(
        kinds("[server]\nport = 1\n", "server = { port = 2 }\n"),
        [DiagnosticKind::TypeMismatch {
            expected: TomlType::Table,
            found: TomlType::InlineTable,
        }]
    );
}

#[test]
fn a_nested_key_is_reported_by_its_whole_path() {
    let merged = merge("[a.b]\nk = 1\n", "[a.b]\nk = 1\nstray = 2\n").unwrap();
    assert_eq!(merged.report.diagnostics()[0].path.to_string(), "a.b.stray");
}

#[test]
fn a_quoted_segment_holding_a_dot_stays_one_segment() {
    let merged = merge("[a]\nk = 1\n", "[a]\nk = 1\n\"b.c\" = 2\n").unwrap();
    assert_eq!(merged.report.diagnostics()[0].path.to_string(), "a.\"b.c\"");
}

#[test]
fn positions_point_at_the_key_in_the_user_source() {
    let merged = merge("a = 1\n", "a = 1\n\n  stray = 2\n").unwrap();
    let diagnostic = &merged.report.diagnostics()[0];
    assert_eq!((diagnostic.line, diagnostic.column), (3, 3));
}

#[test]
fn a_position_counts_characters_not_bytes() {
    let merged = merge("a = 1\n", "# ✂ a snip\nstray = 2\n").unwrap();
    let diagnostic = &merged.report.diagnostics()[0];
    assert_eq!((diagnostic.line, diagnostic.column), (2, 1));
}

#[test]
fn a_document_the_merge_agrees_with_reports_nothing() {
    assert!(kinds("##: doc\na = 1\n", "a = 2\n").is_empty());
}

#[test]
fn a_type_mismatch_deeper_in_a_table_is_still_an_error() {
    let merged = merge("[a]\nk = 1\n", "[a]\nk = true\n").unwrap();
    assert!(merged.report.has_errors());
    assert_eq!(merged.report.diagnostics()[0].path.to_string(), "a.k");
}
