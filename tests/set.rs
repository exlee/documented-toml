//! Setting a value in the person's file.

use documented_toml::{DiagnosticKind, Error, MergeOptions, Newline, merge};

/// Merging the result again must not move it.
fn settled(defaults: &str, text: &str) {
    let again = merge(defaults, text).unwrap().to_toml_string();
    assert_eq!(again, text, "merging the output again moved it");
}

#[test]
fn a_value_lands_where_the_defaults_put_its_key() {
    let defaults = "##: First.\nfirst = 1\n\n##: Second.\nsecond = 2\n\n##: Third.\nthird = 3\n";
    let mut merged = merge(defaults, "").unwrap();
    merged.set("second", 20).unwrap();
    let text = merged.to_toml_string();
    assert_eq!(
        text,
        "##: First.\n#: first = 1\n\n##: Second.\n#: second = 2\nsecond = 20\n\n##: Third.\n#: third = 3\n"
    );
    settled(defaults, &text);
}

#[test]
fn a_value_lands_in_the_section_the_defaults_declare_it_in() {
    let defaults = "top = 1\n\n##: The database.\n[db]\nhost = \"localhost\"\nport = 5432\n";
    let mut merged = merge(defaults, "").unwrap();
    merged.set("db.port", 6000).unwrap();
    let text = merged.to_toml_string();
    assert_eq!(
        text,
        "#: top = 1\n\n##: The database.\n[db]\n#: host = \"localhost\"\n#: port = 5432\nport = 6000\n"
    );
    settled(defaults, &text);
}

#[test]
fn setting_a_value_keeps_the_comments_the_person_wrote_above_it() {
    let defaults = "##: How long.\ntimeout = 30\n";
    let user = "# the staging server is slow\ntimeout = 120\n";
    let mut merged = merge(defaults, user).unwrap();
    merged.set("timeout", 200).unwrap();
    assert_eq!(
        merged.to_toml_string(),
        "##: How long.\n#: timeout = 30\n# the staging server is slow\ntimeout = 200\n"
    );
}

#[test]
fn setting_the_shipped_value_records_no_default_above_it() {
    let defaults = "##: How many.\ncount = 1\n";
    let mut merged = merge(defaults, "").unwrap();
    merged.set("count", 1).unwrap();
    let text = merged.to_toml_string();
    assert_eq!(text, "##: How many.\ncount = 1\n");
    assert!(merged.user_set("count"));
    settled(defaults, &text);
}

#[test]
fn a_key_the_defaults_do_not_declare_is_kept_and_reported() {
    let mut merged = merge("declared = 1\n", "").unwrap();
    merged.set("mine", "here").unwrap();
    assert!(merged.to_toml_string().contains("mine = \"here\""));
    let kinds: Vec<_> = merged
        .report
        .diagnostics()
        .iter()
        .map(|d| &d.kind)
        .collect();
    assert!(
        matches!(kinds.as_slice(), [DiagnosticKind::UnknownKey]),
        "{kinds:?}"
    );
}

#[test]
fn a_value_of_the_wrong_type_is_kept_and_reported() {
    let mut merged = merge("count = 1\n", "").unwrap();
    merged.set("count", "twelve").unwrap();
    assert!(merged.to_toml_string().contains("count = \"twelve\""));
    assert!(merged.report.has_errors());
    assert!(matches!(
        merged.report.diagnostics().first().map(|d| &d.kind),
        Some(DiagnosticKind::TypeMismatch { .. })
    ));
}

#[test]
fn a_quoted_segment_holding_a_dot_is_one_segment() {
    let defaults = "[table]\n\"dotted.name\" = 1\n";
    let mut merged = merge(defaults, "").unwrap();
    merged.set("table.\"dotted.name\"", 9).unwrap();
    let text = merged.to_toml_string();
    assert!(text.contains("\"dotted.name\" = 9"), "{text}");
    assert!(merged.user_set("table.\"dotted.name\""));
    settled(defaults, &text);
}

#[test]
fn a_path_where_a_table_sits_is_refused() {
    let mut merged = merge("[db]\nhost = \"localhost\"\n", "[db]\nhost = \"mine\"\n").unwrap();
    let error = merged.set("db", 1).unwrap_err();
    assert!(matches!(error, Error::NotAValue { .. }), "{error:?}");
    // Refused means unchanged.
    assert!(merged.to_toml_string().contains("host = \"mine\""));
}

#[test]
fn a_path_that_does_not_parse_is_refused() {
    let mut merged = merge("a = 1\n", "").unwrap();
    assert!(matches!(
        merged.set("not a path", 1).unwrap_err(),
        Error::SetPath { .. }
    ));
}

#[test]
fn the_persons_line_ending_survives_a_set() {
    let mut merged = merge("a = 1\nb = 2\n", "a = 5\r\n").unwrap();
    assert_eq!(merged.newline(), Newline::CrLf);
    merged.set("b", 6).unwrap();
    let text = merged.to_toml_string();
    assert!(!text.contains("\n\r"), "{text:?}");
    assert_eq!(text.matches("\r\n").count(), text.matches('\n').count());
    assert!(text.contains("b = 6"));
}

#[test]
fn set_follows_the_options_the_merge_was_made_with() {
    let options = MergeOptions::new()
        .defaults_commented(false)
        .markers("#|", "#=");
    let defaults = "#| How many.\ncount = 1\nother = 2\n";
    let mut merged = options.merge(defaults, "").unwrap();
    merged.set("count", 7).unwrap();
    let text = merged.to_toml_string();
    // Live defaults, so `other` stays a key; the custom marker is used.
    assert_eq!(text, "#| How many.\n#= count = 1\ncount = 7\nother = 2\n");
}

#[test]
fn setting_a_value_twice_leaves_the_last_one() {
    let defaults = "a = 1\n";
    let mut merged = merge(defaults, "").unwrap();
    merged.set("a", 2).unwrap();
    merged.set("a", 3).unwrap();
    let text = merged.to_toml_string();
    assert_eq!(text, "#: a = 1\na = 3\n");
    settled(defaults, &text);
}
