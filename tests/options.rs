//! The marker, the documents the merge refuses, and what comes out of a merge.

use toml_merge::{DEFAULT_PROSE_MARKER, DEFAULT_SAMPLE_MARKER, Error, MergeOptions, merge};

#[test]
fn the_default_markers_are_the_documented_ones() {
    assert_eq!(MergeOptions::new().prose_marker(), DEFAULT_PROSE_MARKER);
    assert_eq!(MergeOptions::new().sample_marker(), DEFAULT_SAMPLE_MARKER);
}

#[test]
fn different_markers_own_the_lines_the_default_ones_would_not() {
    let merged = MergeOptions::new()
        .markers("#!!", "#!")
        .merge("#!! Doc.\n##: not mine\ncount = 1\n", "count = 2\n")
        .unwrap();
    assert_eq!(
        merged.to_toml_string(),
        "#!! Doc.\n#! count = 1\ncount = 2\n"
    );
}

#[test]
fn with_different_markers_the_old_ones_are_the_persons_text() {
    let merged = MergeOptions::new()
        .markers("#!!", "#!")
        .merge("count = 1\n", "##: an ordinary comment now\ncount = 2\n")
        .unwrap();
    assert_eq!(
        merged.to_toml_string(),
        "#! count = 1\n##: an ordinary comment now\ncount = 2\n"
    );
}

#[test]
fn a_sample_for_an_option_with_no_value_stays_commented_out() {
    // `optional` is documented but ships no value, so its block stays as
    // written. `declared` ships one, and the person's departure is recorded
    // under its own prose.
    let merged = merge(
        "##: What it does.\n#: optional = \"never set\"\n\ndeclared = 1\n",
        "declared = 2\n",
    )
    .unwrap();
    assert_eq!(
        merged.to_toml_string(),
        "##: What it does.\n#: optional = \"never set\"\n\n#: declared = 1\ndeclared = 2\n"
    );
}

#[test]
fn a_key_the_defaults_anchor_documentation_on_is_not_unknown() {
    let merged = merge(
        "declared = 1\n\n##: How accounts are written:\n#: [[accounts]]\n#: name = \"Personal\"\n",
        "declared = 1\n\n[[accounts]]\nname = \"Mine\"\n",
    )
    .unwrap();
    assert!(merged.report.diagnostics().is_empty());
    assert!(
        merged
            .to_toml_string()
            .contains("#: name = \"Personal\"\n\n[[accounts]]"),
        "{}",
        merged.to_toml_string()
    );
}

#[test]

fn a_default_document_that_does_not_parse_is_named_as_the_broken_one() {
    assert!(matches!(
        merge("a = \n", "a = 1\n"),
        Err(Error::DefaultParse { .. })
    ));
}

#[test]
fn a_user_document_that_does_not_parse_is_named_as_the_broken_one() {
    assert!(matches!(
        merge("a = 1\n", "a = \n"),
        Err(Error::UserParse { .. })
    ));
}

#[test]
fn defaults_that_declare_no_keys_are_an_application_bug() {
    assert!(matches!(
        merge("# only a note\n", "a = 1\n"),
        Err(Error::DefaultsDeclareNoKeys)
    ));
}

#[test]
fn a_zero_byte_default_document_is_allowed() {
    let merged = merge("", "a = 1\n").unwrap();
    assert_eq!(merged.to_toml_string(), "a = 1\n");
}

#[test]
fn the_merged_document_is_also_the_effective_configuration() {
    let merged = merge("##: Doc.\nadded = 3\nkept = 1\n", "kept = 7\n").unwrap();
    let document = merged.document();
    assert_eq!(document["added"].as_integer(), Some(3));
    assert_eq!(document["kept"].as_integer(), Some(7));
    assert_eq!(merged.to_toml_string(), merged.into_document().to_string());
}

#[test]
fn every_default_key_is_materialised_with_a_live_value() {
    let merged = merge("[a]\nb = 1\n\n[[c]]\nd = 2\n", "").unwrap();
    let document = merged.document();
    assert_eq!(document["a"]["b"].as_integer(), Some(1));
    assert_eq!(document["c"][0]["d"].as_integer(), Some(2));
}
