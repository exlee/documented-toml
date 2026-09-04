//! Rename rules: what they move, when they decline to, and what they report.

use documented_toml::{DiagnosticKind, DottedPath, MergeOptions};

#[test]
fn a_rule_moves_a_value_to_its_new_path() {
    let merged = MergeOptions::new()
        .migrate("server.timeout", "network.timeout_seconds")
        .merge(
            "##: Seconds to wait.\n[network]\ntimeout_seconds = 30\n",
            "[server]\ntimeout = 99\n",
        )
        .unwrap();
    let document = merged.document();
    assert_eq!(
        document["network"]["timeout_seconds"].as_integer(),
        Some(99)
    );
}

#[test]
fn the_moved_value_takes_part_in_the_merge_as_if_it_were_written_there() {
    let merged = MergeOptions::new()
        .migrate("old", "new")
        .merge("##: Doc.\nnew = 1\n", "old = 7\n")
        .unwrap();
    assert!(
        merged.to_toml_string().contains("#: new = 1\nnew = 7\n"),
        "{}",
        merged.to_toml_string()
    );
}

#[test]
fn the_person_keeps_the_comments_they_wrote_above_the_old_key() {
    let merged = MergeOptions::new()
        .migrate("old", "new")
        .merge("new = 1\n", "# why I set this\nold = 7\n")
        .unwrap();
    assert!(
        merged
            .to_toml_string()
            .contains("# why I set this\nnew = 7\n"),
        "{}",
        merged.to_toml_string()
    );
}

#[test]
fn a_move_is_reported_with_the_path_it_came_from() {
    let merged = MergeOptions::new()
        .migrate("old", "new")
        .merge("new = 1\n", "old = 7\n")
        .unwrap();
    assert_eq!(
        merged.report.diagnostics()[0].kind,
        DiagnosticKind::Migrated {
            from: DottedPath::parse("old").unwrap()
        }
    );
    assert!(!merged.report.has_errors());
}

#[test]
fn an_explicit_value_at_the_new_path_wins() {
    let merged = MergeOptions::new()
        .migrate("old", "new")
        .merge("new = 1\n", "old = 7\nnew = 3\n")
        .unwrap();
    assert_eq!(merged.document()["new"].as_integer(), Some(3));
    let kinds: Vec<_> = merged
        .report
        .diagnostics()
        .iter()
        .map(|d| d.kind.clone())
        .collect();
    assert_eq!(kinds, [DiagnosticKind::UnknownKey]);
}

#[test]
fn a_rule_whose_old_path_is_absent_does_nothing() {
    let merged = MergeOptions::new()
        .migrate("old", "new")
        .merge("new = 1\n", "new = 2\n")
        .unwrap();
    assert!(merged.report.diagnostics().is_empty());
}

#[test]
fn a_rule_creates_the_tables_above_the_new_path() {
    let merged = MergeOptions::new()
        .migrate("timeout", "network.deep.timeout")
        .merge("[network.deep]\ntimeout = 1\n", "timeout = 9\n")
        .unwrap();
    assert_eq!(
        merged.document()["network"]["deep"]["timeout"].as_integer(),
        Some(9)
    );
}

#[test]
fn an_unreadable_path_fails_the_merge_rather_than_the_builder() {
    let options = MergeOptions::new().migrate("a..b", "c");
    assert!(options.merge("c = 1\n", "c = 1\n").is_err());
}

#[test]
fn rules_run_in_the_order_they_were_added() {
    let merged = MergeOptions::new()
        .migrate("a", "b")
        .migrate("b", "c")
        .merge("c = 1\n", "a = 9\n")
        .unwrap();
    assert_eq!(merged.document()["c"].as_integer(), Some(9));
}
