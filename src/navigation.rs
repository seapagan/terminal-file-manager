//! Native ancestor targets never round-trip through displayed text.
use std::path::{Path, PathBuf};

/// A displayed slice and its exact native navigation target.
#[derive(Clone, Debug)]
pub struct PathSegment {
    pub text: String,
    pub path: PathBuf,
}

/// Lexical ancestry preserves symlinks and native non-Unicode spelling.
pub fn path_segments(path: &Path) -> Vec<PathSegment> {
    let mut ancestors = path
        .ancestors()
        .filter(|p| !p.as_os_str().is_empty())
        .collect::<Vec<_>>();
    ancestors.reverse();
    let mut previous = 0;
    ancestors
        .into_iter()
        .map(|ancestor| {
            let display = crate::ui::clean(&ancestor.to_string_lossy());
            let text = display[previous..].to_owned();
            previous = display.len();
            PathSegment {
                text,
                path: ancestor.into(),
            }
        })
        .collect()
}

/// Session-only full-path picker, bound to the pane/path that opened it.
#[derive(Clone, Debug)]
pub struct PathPicker {
    pub pane: usize,
    pub path: PathBuf,
    pub selected: usize,
    pub scroll: usize,
}
