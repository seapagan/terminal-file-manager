//! Read-only local file browsing with a testable application and terminal boundary.

use std::ffi::OsString;

pub mod app;
pub mod browser;
pub mod color;
pub mod fs;
pub mod input;
pub mod listing;
pub mod settings;
pub mod terminal;
pub mod ui;
pub mod workers;

/// Help for the implemented local browsing contract.
pub const HELP: &str = "Usage: tfm [DIRECTORY]\n       tfm [--help | --version]\n\nBrowse local files read-only. Starts in the shell's current directory,\nor the explicit DIRECTORY. Requires an interactive terminal.\nFiles are never launched; mutations, SSH and viewers are unavailable in M1.\n\nOptions:\n  -h, --help     Print help\n  -V, --version  Print version\n  -- DIRECTORY  Browse a directory whose name starts with a hyphen\n\nInside the browser: ? for shortcuts, q/F10 to quit.";

/// Parsed startup intent, preserving native directory identity.
#[derive(Debug)]
pub enum Invocation {
    Output(String),
    Browse(std::path::PathBuf),
}

/// Return output for the implemented CLI arguments.
///
/// # Errors
///
/// Returns an error for unsupported options or multiple paths. Native path arguments
/// need not be UTF-8.
///
/// ```
/// use std::ffi::OsString;
/// let result = terminal_file_manager::run(&[OsString::from("--version")]).unwrap();
/// assert!(matches!(result, terminal_file_manager::Invocation::Output(text)
///     if text == format!("tfm {}", env!("CARGO_PKG_VERSION"))));
/// ```
pub fn run(arguments: &[OsString]) -> Result<Invocation, &'static str> {
    match arguments {
        [] => Ok(Invocation::Browse(".".into())),
        [argument] if argument == "--help" || argument == "-h" => {
            Ok(Invocation::Output(HELP.to_owned()))
        }
        [argument] if argument == "--version" || argument == "-V" => Ok(Invocation::Output(
            format!("tfm {}", env!("CARGO_PKG_VERSION")),
        )),
        [argument] if !argument.as_encoded_bytes().starts_with(b"-") => {
            Ok(Invocation::Browse(argument.into()))
        }
        [separator, path] if separator == "--" => Ok(Invocation::Browse(path.into())),
        _ => Err("unsupported arguments; use --help"),
    }
}
