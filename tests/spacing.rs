//! Blank lines between keys and sections. Alignment is off here so the
//! spacing stands on its own.

use documented_toml::{MergeOptions, Merged};

fn merge(defaults: &str, user: &str) -> Merged {
    MergeOptions::new()
        .align_values(false)
        .merge(defaults, user)
        .unwrap()
}

fn check(defaults: &str, user: &str, expected: &str) {
    let merged = merge(defaults, user).to_toml_string();
    assert_eq!(merged, expected);
    assert_eq!(merge(defaults, &merged).to_toml_string(), merged);
}

#[test]
fn separates_sections_and_their_documentation() {
    let defaults = "enabled = true\n##: Network settings.\n[network]\nport = 80\n[network.tls]\nenabled = true\n[[accounts]]\nname = 'one'\n[[accounts]]\nname = 'two'\n";
    let expected = "enabled = true\n\n##: Network settings.\n[network]\nport = 80\n\n[network.tls]\nenabled = true\n\n[[accounts]]\nname = 'one'\n\n[[accounts]]\nname = 'two'\n";
    check(defaults, "", expected);
    check(defaults, defaults, expected);
}

#[test]
fn preserves_default_blank_lines_with_and_without_comments() {
    let defaults = "first = 1\n\n \nsecond = 2\n\n\n##: Third option.\nthird = 3\n";
    check(defaults, "", defaults);
    check(defaults, "first = 1\nsecond = 2\nthird = 3\n", defaults);
}

#[test]
fn preserves_comment_paragraphs_and_indentation() {
    let defaults = "##: Run this:\n##:   command  --flag\n##:\n##:     nested command\nvalue = 1\n";
    let user = "# First paragraph.\n\n \n#   Indented second paragraph.\n\nvalue = 1\n";
    check(
        defaults,
        user,
        &format!("{}{}", defaults.strip_suffix("value = 1\n").unwrap(), user),
    );
}

#[test]
fn preserves_user_section_spacing() {
    let user = "[first]\nvalue = 1\n\n \n# Second section.\n[second]\nvalue = 2\n";
    check("[first]\nvalue = 1\n[second]\nvalue = 2\n", user, user);
}

#[test]
fn first_nested_section_has_no_added_leading_blank() {
    let defaults = "[outer.inner]\nvalue = 1\n";
    check(defaults, "", defaults);
}

#[test]
fn dotted_keys_do_not_introduce_section_spacing() {
    let defaults = "first.value = 1\nsecond.value = 2\n";
    check(defaults, "", defaults);
}

#[test]
fn preserves_spacing_in_optional_examples() {
    let defaults = "##: Commands to run.\n#: commands = [\n#:   'echo  hello',\n#:   'printf    world',\n#: ]\n";
    let first = merge(defaults, "").to_toml_string();
    assert!(first.contains("#:   'echo  hello',\n#:   'printf    world',\n"));
    check(defaults, &first, &first);
}

#[test]
fn separates_unknown_sections_with_crlf() {
    let user = "[one]\r\nvalue = 1\r\n# Another section.\r\n[two]\r\nvalue = 2\r\n";
    let expected = "[one]\r\nvalue = 1\r\n\r\n# Another section.\r\n[two]\r\nvalue = 2\r\n";
    check("", user, expected);
}
