//! Minimal CLI boundary for the M0-B foundation.

use std::ffi::OsString;

pub mod app;
pub mod fs;
pub mod listing;

/// Help for the currently implemented scaffold.
pub const HELP: &str = "Usage: tfm [--help | --version]\n\nFile management is not implemented yet.\n\nOptions:\n  -h, --help     Print help\n  -V, --version  Print version";

/// Return output for the implemented CLI arguments.
///
/// # Errors
///
/// Returns an error for any unsupported arguments, including non-UTF8 arguments.
///
/// ```
/// use std::ffi::OsString;
/// assert_eq!(
///     terminal_file_manager::run(&[OsString::from("--version")]).unwrap(),
///     format!("tfm {}", env!("CARGO_PKG_VERSION")),
/// );
/// ```
pub fn run(arguments: &[OsString]) -> Result<String, &'static str> {
    match arguments {
        [] => Ok(HELP.to_owned()),
        [argument] if argument == "--help" || argument == "-h" => Ok(HELP.to_owned()),
        [argument] if argument == "--version" || argument == "-V" => {
            Ok(format!("tfm {}", env!("CARGO_PKG_VERSION")))
        }
        _ => Err("unsupported arguments; use --help"),
    }
}
