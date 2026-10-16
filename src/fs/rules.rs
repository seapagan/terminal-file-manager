//! lsplus rule precedence, rebuilt per refresh to avoid a lifetime-stale cache.

use ignore::{
    Match,
    gitignore::{Gitignore, GitignoreBuilder},
};
use std::{
    fs, io,
    path::{Path, PathBuf},
};

/// Explicit global-rule input for isolated tests; production may discover Git config.
#[derive(Clone, Debug)]
pub struct IgnoreContext {
    pub global_file: Option<PathBuf>,
    pub discover_global: bool,
}

impl IgnoreContext {
    /// Do not consult the developer's home or Git configuration.
    pub fn isolated() -> Self {
        Self {
            global_file: None,
            discover_global: false,
        }
    }
}

impl Default for IgnoreContext {
    fn default() -> Self {
        Self {
            global_file: None,
            discover_global: true,
        }
    }
}

pub(super) struct Rules {
    root: PathBuf,
    matchers: Vec<Gitignore>,
}

impl Rules {
    pub(super) fn ignored(&self, path: &Path, directory: bool) -> bool {
        let Ok(relative) = path.strip_prefix(&self.root) else {
            return false;
        };
        let mut current = self.root.clone();
        for component in relative.components() {
            current.push(component);
            if self.ignored_at(&current, current != path || directory) {
                return true;
            }
        }
        false
    }

    fn ignored_at(&self, path: &Path, directory: bool) -> bool {
        for rules in self.matchers.iter().rev() {
            if !path.starts_with(rules.path()) {
                continue;
            }
            match rules.matched(path, directory) {
                Match::Ignore(_) => return true,
                Match::Whitelist(_) => return false,
                Match::None => {}
            }
        }
        false
    }
}

pub(super) fn load(directory: &Path, context: &IgnoreContext) -> (Option<Rules>, Vec<String>) {
    match build(directory, context) {
        Ok(rules) => (Some(rules), Vec::new()),
        Err(error) => (None, vec![format!("Git ignore status unknown: {error}")]),
    }
}

fn build(directory: &Path, context: &IgnoreContext) -> Result<Rules, String> {
    let Some((root, git)) = repository(directory).map_err(|e| e.to_string())? else {
        return Ok(Rules {
            root: directory.to_path_buf(),
            matchers: Vec::new(),
        });
    };
    let mut matchers = Vec::new();
    if context.discover_global {
        let (global, error) = GitignoreBuilder::new(&root).build_global();
        if let Some(error) = error {
            return Err(error.to_string());
        }
        matchers.push(global);
    }
    if let Some(global) = &context.global_file {
        matchers.push(matcher(&root, global)?);
    }
    matchers.push(matcher(&root, &git.join("info/exclude"))?);
    let mut parents: Vec<_> = directory
        .ancestors()
        .take_while(|p| p.starts_with(&root))
        .collect();
    parents.reverse();
    for parent in parents {
        matchers.push(matcher(parent, &parent.join(".gitignore"))?);
    }
    Ok(Rules { root, matchers })
}

fn matcher(root: &Path, file: &Path) -> Result<Gitignore, String> {
    let mut builder = GitignoreBuilder::new(root);
    match fs::symlink_metadata(file) {
        Ok(_) => {
            if let Some(error) = builder.add(file) {
                return Err(error.to_string());
            }
        }
        Err(error) if error.kind() == io::ErrorKind::NotFound => {}
        Err(error) => return Err(error.to_string()),
    }
    builder.build().map_err(|e| e.to_string())
}

fn repository(directory: &Path) -> io::Result<Option<(PathBuf, PathBuf)>> {
    for parent in directory.ancestors() {
        let dot = parent.join(".git");
        let metadata = match fs::symlink_metadata(&dot) {
            Ok(metadata) => metadata,
            Err(error) if error.kind() == io::ErrorKind::NotFound => continue,
            Err(error) => return Err(error),
        };
        let git = if metadata.is_dir() {
            dot
        } else {
            gitdir(&dot)?
        };
        let common = match fs::read_to_string(git.join("commondir")) {
            Ok(text) => git.join(text.trim()),
            Err(error) if error.kind() == io::ErrorKind::NotFound => git,
            Err(error) => return Err(error),
        };
        return Ok(Some((parent.to_path_buf(), common)));
    }
    Ok(None)
}

fn gitdir(dot: &Path) -> io::Result<PathBuf> {
    let text = fs::read_to_string(dot)?;
    let relative = text
        .strip_prefix("gitdir:")
        .ok_or_else(|| io::Error::other("invalid .git file"))?
        .trim();
    Ok(dot.parent().unwrap_or(Path::new(".")).join(relative))
}
