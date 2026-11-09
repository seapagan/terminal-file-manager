//! Read-only local file browsing with a testable application and terminal boundary.

use std::ffi::OsString;

pub mod app;
pub mod browser;
pub mod color;
pub mod fs;
pub mod fuzzy_time;
pub mod input;
pub mod listing;
pub mod notifications;
pub mod path_copy;
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
pub mod workflow;

/// Help for the implemented local browsing contract.
pub const HELP: &str =
    "Manage local files in one or two panes. The current directory is the default.

Usage: tfm [OPTIONS] [DIRECTORY [OTHER_DIRECTORY]]
       tfm --help | --version

Options:
  -2  --two-panes        Start with two panes
      --save-session     Save selected locations/layout and exit
      --restore-session  Restore saved locations/layout; no directory arguments
  -h  --help             Print help and exit
  -V  --version          Print version and exit
      --                 Treat following arguments as directory paths

In the browser, press ? or F1 for interactive help.";

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
///     if text.contains(env!("CARGO_PKG_VERSION"))));
/// ```
pub fn run(arguments: &[OsString]) -> Result<Invocation, &'static str> {
    match arguments {
        [] => Ok(Invocation::BrowseOptions(startup::Startup::default())),
        [argument] => single_argument(argument),
        [separator, path] if separator == "--" => Ok(Invocation::Browse(path.into())),
        _ => startup::parse(arguments),
    }
}

fn single_argument(argument: &OsString) -> Result<Invocation, &'static str> {
    match argument.as_encoded_bytes() {
        b"--help" | b"-h" => Ok(Invocation::Output(help())),
        b"--version" | b"-V" => Ok(Invocation::Output(version())),
        bytes if !bytes.starts_with(b"-") => Ok(Invocation::Browse(argument.into())),
        _ => startup::parse(std::slice::from_ref(argument)),
    }
}

fn version() -> String {
    use colored_text::{Colorize, RenderTarget};
    let licence = env!("CARGO_PKG_LICENSE")
        .split_whitespace()
        .map(|token| match token {
            "OR" | "AND" => token.to_owned(),
            _ => token.green().render(RenderTarget::Stdout),
        })
        .collect::<Vec<_>>()
        .join(" ");
    format!(
        "{} {}\nCopyright © 2026 Grant Ramsay\nLicensed under {licence}",
        "tfm".cyan().bold().render(RenderTarget::Stdout),
        env!("CARGO_PKG_VERSION")
    )
}

fn help() -> String {
    use colored_text::{Colorize, RenderTarget};
    let tokens = regex::Regex::new(
        r"(?m)^(?:Usage:|Options:)|--[a-z-]+|--|-[2hV]|\b(?:OTHER_)?DIRECTORY\b|\?|\bF1\b",
    )
    .unwrap();
    tokens
        .replace_all(HELP, |captures: &regex::Captures<'_>| {
            let text = &captures[0];
            let styled = match text {
                "Usage:" | "Options:" => text.cyan().bold(),
                "DIRECTORY" | "OTHER_DIRECTORY" | "?" | "F1" => text.cyan(),
                _ => text.green(),
            };
            styled.render(RenderTarget::Stdout)
        })
        .into_owned()
}

/// Cached native ownership and presentation policy.
pub mod owner;

/// Frozen local operation requests and shared mutation coordination.
pub mod operations;

pub mod navigation;
