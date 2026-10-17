//! Native path startup options, separate from saved view policy.
use crate::Invocation;
use std::{ffi::OsString, path::PathBuf};

/// Explicit command-line startup intent. No lossy path conversion.
#[derive(Clone, Debug, Default)]
pub struct Startup {
    pub paths: Vec<PathBuf>,
    pub two_panes: bool,
}

pub(crate) fn parse(arguments: &[OsString]) -> Result<Invocation, &'static str> {
    let mut startup = Startup::default();
    let mut positional = false;
    for argument in arguments {
        if positional {
            startup.paths.push(argument.into());
            continue;
        }
        match argument.as_encoded_bytes() {
            b"--" => positional = true,
            b"-2" | b"--two-panes" => startup.two_panes = true,
            bytes if bytes.starts_with(b"-") => return Err("unsupported arguments; use --help"),
            _ => startup.paths.push(argument.into()),
        }
    }
    if startup.paths.len() > 2 {
        return Err("unsupported arguments; use --help");
    }
    if !startup.two_panes && startup.paths.len() <= 1 {
        Ok(Invocation::Browse(
            startup.paths.pop().unwrap_or_else(|| ".".into()),
        ))
    } else {
        Ok(Invocation::BrowseOptions(startup))
    }
}

impl Startup {
    /// Duplicate the one explicit location; two explicit locations imply dual panes.
    pub fn locations(&self) -> [PathBuf; 2] {
        let left = self.paths.first().cloned().unwrap_or_else(|| ".".into());
        let right = self.paths.get(1).cloned().unwrap_or_else(|| left.clone());
        [left, right]
    }
}
