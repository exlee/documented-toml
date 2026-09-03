//! The `toml-merge` command.
//!
//! ```text
//! toml-merge merge --default D.toml --user U.toml [--in-place | --output OUT]
//! toml-merge check --default D.toml --user U.toml
//! ```

use std::fs;
use std::io::Write;
use std::path::{Path, PathBuf};
use std::process::ExitCode;

use toml_merge::{MergeOptions, Merged};

const USAGE: &str = "\
usage:
  toml-merge merge --default D.toml --user U.toml [--in-place | --output OUT]
  toml-merge check --default D.toml --user U.toml

  merge  writes the merged document, to stdout unless --output or --in-place
  check  writes nothing

Both print diagnostics to stderr and exit non-zero when one of them is an error.
";

fn main() -> ExitCode {
    match run() {
        Ok(code) => code,
        Err(message) => {
            eprintln!("toml-merge: {message}");
            ExitCode::from(2)
        }
    }
}

fn run() -> Result<ExitCode, String> {
    let args: Vec<String> = std::env::args().skip(1).collect();
    let invocation = Invocation::parse(&args)?;
    let default_src = read(&invocation.default)?;
    let user_src = read(&invocation.user)?;
    let merged = MergeOptions::new()
        .merge(&default_src, &user_src)
        .map_err(|e| e.to_string())?;

    report(&merged, &invocation.user);
    if invocation.command == Command::Merge {
        write_output(&merged, &invocation)?;
    }
    Ok(if merged.report.has_errors() {
        ExitCode::FAILURE
    } else {
        ExitCode::SUCCESS
    })
}

fn read(path: &Path) -> Result<String, String> {
    fs::read_to_string(path).map_err(|e| format!("{}: {e}", path.display()))
}

fn report(merged: &Merged, user: &Path) {
    for diagnostic in merged.report.diagnostics() {
        eprintln!(
            "{}:{}:{}: {}: {}: {}",
            user.display(),
            diagnostic.line,
            diagnostic.column,
            diagnostic.severity,
            diagnostic.path,
            diagnostic.kind
        );
    }
}

/// Writes the merged document where the invocation asks for it.
///
/// `--in-place` writes a sibling temporary file and renames it over the target,
/// so an interrupted run cannot leave a truncated config behind.
fn write_output(merged: &Merged, invocation: &Invocation) -> Result<(), String> {
    let text = merged.to_toml_string();
    match &invocation.destination {
        Destination::Stdout => {
            print!("{text}");
            Ok(())
        }
        Destination::File(path) => {
            fs::write(path, text).map_err(|e| format!("{}: {e}", path.display()))
        }
        Destination::InPlace => replace(&invocation.user, &text),
    }
}

fn replace(target: &Path, text: &str) -> Result<(), String> {
    let name = target
        .file_name()
        .ok_or_else(|| format!("{}: not a file", target.display()))?;
    let mut temporary = target.to_path_buf();
    temporary.set_file_name(format!(
        ".{}.toml-merge.{}",
        name.to_string_lossy(),
        std::process::id()
    ));
    let write = || -> std::io::Result<()> {
        let mut file = fs::File::create(&temporary)?;
        file.write_all(text.as_bytes())?;
        file.sync_all()
    };
    if let Err(e) = write() {
        let _ = fs::remove_file(&temporary);
        return Err(format!("{}: {e}", temporary.display()));
    }
    fs::rename(&temporary, target).map_err(|e| {
        let _ = fs::remove_file(&temporary);
        format!("{}: {e}", target.display())
    })
}

#[derive(Debug, PartialEq, Eq)]
enum Command {
    Merge,
    Check,
}

#[derive(Debug, PartialEq, Eq)]
enum Destination {
    Stdout,
    File(PathBuf),
    InPlace,
}

#[derive(Debug, PartialEq, Eq)]
struct Invocation {
    command: Command,
    default: PathBuf,
    user: PathBuf,
    destination: Destination,
}

impl Invocation {
    fn parse(args: &[String]) -> Result<Self, String> {
        let mut arguments = args.iter();
        let command = match arguments.next().map(String::as_str) {
            Some("merge") => Command::Merge,
            Some("check") => Command::Check,
            Some("--help" | "-h") | None => return Err(USAGE.to_owned()),
            Some(other) => return Err(format!("no such command: {other}\n\n{USAGE}")),
        };

        let mut default = None;
        let mut user = None;
        let mut destination = Destination::Stdout;
        while let Some(argument) = arguments.next() {
            let mut value = |name: &str| {
                arguments
                    .next()
                    .cloned()
                    .ok_or_else(|| format!("{name} needs a path"))
            };
            match argument.as_str() {
                "--default" => default = Some(PathBuf::from(value("--default")?)),
                "--user" => user = Some(PathBuf::from(value("--user")?)),
                "--output" => destination = Destination::File(PathBuf::from(value("--output")?)),
                "--in-place" => destination = Destination::InPlace,
                other => return Err(format!("no such option: {other}\n\n{USAGE}")),
            }
        }

        let invocation = Self {
            command,
            default: default.ok_or("--default is required")?,
            user: user.ok_or("--user is required")?,
            destination,
        };
        if invocation.command == Command::Check && invocation.destination != Destination::Stdout {
            return Err("check writes nothing, so it takes no --output or --in-place".to_owned());
        }
        Ok(invocation)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn parse(args: &[&str]) -> Result<Invocation, String> {
        Invocation::parse(&args.iter().map(|a| (*a).to_owned()).collect::<Vec<_>>())
    }

    #[test]
    fn merge_defaults_to_stdout() {
        let invocation = parse(&["merge", "--default", "d.toml", "--user", "u.toml"]).unwrap();
        assert_eq!(invocation.command, Command::Merge);
        assert_eq!(invocation.destination, Destination::Stdout);
    }

    #[test]
    fn output_and_in_place_are_destinations() {
        let to_file = parse(&["merge", "--default", "d", "--user", "u", "--output", "o"]).unwrap();
        assert_eq!(to_file.destination, Destination::File(PathBuf::from("o")));
        let over_target = parse(&["merge", "--default", "d", "--user", "u", "--in-place"]).unwrap();
        assert_eq!(over_target.destination, Destination::InPlace);
    }

    #[test]
    fn check_takes_no_destination() {
        let error = parse(&["check", "--default", "d", "--user", "u", "--in-place"]).unwrap_err();
        assert!(error.contains("writes nothing"), "{error}");
    }

    #[test]
    fn both_documents_are_required() {
        assert!(
            parse(&["merge", "--user", "u"])
                .unwrap_err()
                .contains("--default")
        );
        assert!(
            parse(&["merge", "--default", "d"])
                .unwrap_err()
                .contains("--user")
        );
    }

    #[test]
    fn an_option_without_its_path_is_an_error() {
        let error = parse(&["merge", "--default"]).unwrap_err();
        assert!(error.contains("needs a path"), "{error}");
    }

    #[test]
    fn an_unknown_command_is_an_error() {
        assert!(
            parse(&["frobnicate"])
                .unwrap_err()
                .contains("no such command")
        );
        let error = parse(&["merge", "--default", "d", "--user", "u", "--wat"]).unwrap_err();
        assert!(error.contains("no such option"), "{error}");
    }
}
