//! Read-only local file browsing with a testable application and terminal boundary.

use std::ffi::OsString;

pub mod app;
pub mod browser;
pub mod color;
pub mod fs;
pub mod fuzzy_time;
pub mod input;
pub mod listing;
pub mod permissions;
pub mod session;
pub mod settings;
pub mod size;
pub mod startup;
pub mod terminal;
pub mod ui;
pub mod view;
pub mod watch;
pub mod workers;

/// Help for the implemented local browsing contract.
pub const HELP: &str = "Usage: tfm [-2 | --two-panes] [DIRECTORY [OTHER_DIRECTORY]]\n       tfm [--help | --version]\n\nManage local files. Starts in the shell's current directory,\nor the explicit DIRECTORY. Requires an interactive terminal.\nFiles are never launched; SSH and viewers remain unavailable.\nPress r to refresh the current pane.\n\nOptions:\n  --save-session     Save selected locations/layout and exit\n  --restore-session  Restore saved navigation (no explicit paths)\n  -2, --two-panes  Start with two panes\n  -h, --help     Print help\n  -V, --version  Print version\n  -- DIRECTORY  Browse a directory whose name starts with a hyphen\n\nInside the browser: ? for shortcuts, F5/F6 copy/move, F7 mkdir, F8 Trash,\nF9 jobs, q/F10 to quit safely, Q to save session and quit.";

/// Parsed startup intent, preserving native directory identity.
#[derive(Debug)]
pub enum Invocation {
    Output(String),
    Browse(std::path::PathBuf),
    BrowseOptions(startup::Startup),
}

/// Return output for the implemented CLI arguments.
///
/// # Errors
///
/// Returns an error for unsupported options or more than two paths. Native path arguments
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
        [] => Ok(Invocation::BrowseOptions(startup::Startup::default())),
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
        _ => startup::parse(arguments),
    }
}

/// Cached native ownership and presentation policy.
pub mod owner;

/// Frozen local operation requests and shared mutation coordination.
pub mod operations;
