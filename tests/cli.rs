//! The `toml-merge` command, run as a person would run it.

use std::fs;
use std::path::{Path, PathBuf};
use std::process::{Command, Output};

const DEFAULTS: &str = "#: How many.\ncount = 1\n";

struct Sandbox {
    directory: PathBuf,
}

impl Sandbox {
    fn new(name: &str) -> Self {
        let directory =
            std::env::temp_dir().join(format!("toml-merge-{name}-{}", std::process::id()));
        let _ = fs::remove_dir_all(&directory);
        fs::create_dir_all(&directory).expect("a writable temporary directory");
        Self { directory }
    }

    fn write(&self, name: &str, text: &str) -> PathBuf {
        let path = self.directory.join(name);
        fs::write(&path, text).expect("a writable file");
        path
    }

    fn run(&self, args: &[&Path]) -> Output {
        Command::new(env!("CARGO_BIN_EXE_toml-merge"))
            .args(args)
            .output()
            .expect("the binary runs")
    }
}

impl Drop for Sandbox {
    fn drop(&mut self) {
        let _ = fs::remove_dir_all(&self.directory);
    }
}

fn arg(text: &str) -> &Path {
    Path::new(text)
}

#[test]
fn merge_writes_the_document_to_stdout() {
    let sandbox = Sandbox::new("stdout");
    let defaults = sandbox.write("d.toml", DEFAULTS);
    let user = sandbox.write("u.toml", "count = 5\n");
    let output = sandbox.run(&[
        arg("merge"),
        arg("--default"),
        &defaults,
        arg("--user"),
        &user,
    ]);
    assert!(output.status.success());
    assert_eq!(
        String::from_utf8_lossy(&output.stdout),
        "#: How many.\n#: count = 1\ncount = 5\n"
    );
    assert_eq!(fs::read_to_string(&user).unwrap(), "count = 5\n");
}

#[test]
fn output_writes_the_document_to_a_file() {
    let sandbox = Sandbox::new("output");
    let defaults = sandbox.write("d.toml", DEFAULTS);
    let user = sandbox.write("u.toml", "count = 5\n");
    let out = sandbox.directory.join("merged.toml");
    let output = sandbox.run(&[
        arg("merge"),
        arg("--default"),
        &defaults,
        arg("--user"),
        &user,
        arg("--output"),
        &out,
    ]);
    assert!(output.status.success());
    assert!(output.stdout.is_empty());
    assert!(fs::read_to_string(&out).unwrap().contains("count = 5"));
}

#[test]
fn in_place_replaces_the_user_file_and_leaves_no_temporary_behind() {
    let sandbox = Sandbox::new("inplace");
    let defaults = sandbox.write("d.toml", DEFAULTS);
    let user = sandbox.write("u.toml", "count = 5\n");
    let output = sandbox.run(&[
        arg("merge"),
        arg("--default"),
        &defaults,
        arg("--user"),
        &user,
        arg("--in-place"),
    ]);
    assert!(output.status.success());
    assert_eq!(
        fs::read_to_string(&user).unwrap(),
        "#: How many.\n#: count = 1\ncount = 5\n"
    );
    let left_behind: Vec<_> = fs::read_dir(&sandbox.directory)
        .unwrap()
        .map(|e| e.unwrap().file_name().to_string_lossy().into_owned())
        .filter(|name| name != "d.toml" && name != "u.toml")
        .collect();
    assert!(left_behind.is_empty(), "{left_behind:?}");
}

#[test]
fn check_writes_nothing() {
    let sandbox = Sandbox::new("check");
    let defaults = sandbox.write("d.toml", DEFAULTS);
    let user = sandbox.write("u.toml", "count = 5\n");
    let output = sandbox.run(&[
        arg("check"),
        arg("--default"),
        &defaults,
        arg("--user"),
        &user,
    ]);
    assert!(output.status.success());
    assert!(output.stdout.is_empty());
    assert_eq!(fs::read_to_string(&user).unwrap(), "count = 5\n");
}

#[test]
fn diagnostics_go_to_stderr() {
    let sandbox = Sandbox::new("warn");
    let defaults = sandbox.write("d.toml", DEFAULTS);
    let user = sandbox.write("u.toml", "count = 5\nstray = 1\n");
    let output = sandbox.run(&[
        arg("check"),
        arg("--default"),
        &defaults,
        arg("--user"),
        &user,
    ]);
    assert!(output.status.success());
    let stderr = String::from_utf8_lossy(&output.stderr);
    assert!(stderr.contains("u.toml:2:1"), "{stderr}");
    assert!(stderr.contains("warning"), "{stderr}");
    assert!(stderr.contains("stray"), "{stderr}");
}

#[test]
fn an_error_in_the_report_exits_non_zero_and_still_writes() {
    let sandbox = Sandbox::new("mismatch");
    let defaults = sandbox.write("d.toml", DEFAULTS);
    let user = sandbox.write("u.toml", "count = \"lots\"\n");
    let output = sandbox.run(&[
        arg("merge"),
        arg("--default"),
        &defaults,
        arg("--user"),
        &user,
        arg("--in-place"),
    ]);
    assert!(!output.status.success());
    assert!(String::from_utf8_lossy(&output.stderr).contains("error"));
    assert!(
        fs::read_to_string(&user)
            .unwrap()
            .contains("count = \"lots\"")
    );
}

#[test]
fn a_missing_file_is_reported_and_nothing_is_written() {
    let sandbox = Sandbox::new("missing");
    let defaults = sandbox.write("d.toml", DEFAULTS);
    let output = sandbox.run(&[
        arg("merge"),
        arg("--default"),
        &defaults,
        arg("--user"),
        arg("/nonexistent/u.toml"),
    ]);
    assert!(!output.status.success());
    assert!(output.stdout.is_empty());
    assert!(String::from_utf8_lossy(&output.stderr).contains("toml-merge:"));
}

#[test]
fn no_arguments_prints_the_usage() {
    let sandbox = Sandbox::new("usage");
    let output = sandbox.run(&[]);
    assert!(!output.status.success());
    assert!(String::from_utf8_lossy(&output.stderr).contains("usage:"));
}
