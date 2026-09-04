//! The line ending the person's file was written with survives the merge.
//!
//! The comment machinery works in lines and writes them back joined with `\n`,
//! so without this a file written with `\r\n` comes back rewritten from top to
//! bottom, every line of it, including the person's own comments.

use documented_toml::{Newline, merge};

#[test]
fn a_file_written_with_crlf_comes_back_with_crlf() {
    let merged = merge(
        "##: Doc.\ncount = 1\n",
        "count = 7\r\n# mine\r\nother = 1\r\n",
    )
    .unwrap();
    let text = merged.to_toml_string();
    assert_eq!(merged.newline(), Newline::CrLf);
    // No line is left ending in a bare LF, the merge's own lines included.
    for line in text.split("\r\n") {
        assert!(!line.contains('\n'), "a line ends with LF: {line:?}");
    }
    assert!(text.ends_with("\r\n"));
}

#[test]
fn the_lines_the_merge_writes_take_the_persons_ending_too() {
    // `#: count = 1` is a line the merge writes itself.
    let merged = merge("##: Doc.\ncount = 1\n", "count = 7\r\n").unwrap();
    assert!(merged.to_toml_string().contains("#: count = 1\r\n"));
}

#[test]
fn a_file_written_with_lf_is_left_with_lf() {
    let merged = merge("##: Doc.\ncount = 1\n", "count = 7\n").unwrap();
    assert_eq!(merged.newline(), Newline::Lf);
    assert!(!merged.to_toml_string().contains('\r'));
}

#[test]
fn the_defaults_decide_nothing_about_the_ending() {
    // The file being written back is the person's, so the ending is theirs.
    let crlf_defaults = merge("##: Doc.\r\ncount = 1\r\n", "count = 7\n").unwrap();
    assert_eq!(crlf_defaults.newline(), Newline::Lf);
    assert!(!crlf_defaults.to_toml_string().contains('\r'));

    let lf_defaults = merge("##: Doc.\ncount = 1\n", "count = 7\r\n").unwrap();
    assert_eq!(lf_defaults.newline(), Newline::CrLf);
}

#[test]
fn a_second_merge_of_the_output_moves_nothing() {
    let user = "count = 7\r\n# mine\r\nother = 1\r\n";
    let once = merge("##: Doc.\ncount = 1\n", user)
        .unwrap()
        .to_toml_string();
    let twice = merge("##: Doc.\ncount = 1\n", &once)
        .unwrap()
        .to_toml_string();
    assert_eq!(once, twice);
}
