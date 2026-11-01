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
        matchers.push(matcher(&root, global, true)?);
    }
    matchers.push(matcher(&root, &git.join("info/exclude"), true)?);
    let mut parents: Vec<_> = directory
        .ancestors()
        .take_while(|p| p.starts_with(&root))
        .collect();
    parents.reverse();
    for parent in parents {
        matchers.push(matcher(parent, &parent.join(".gitignore"), false)?);
    }
    Ok(Rules { root, matchers })
}

fn matcher(root: &Path, file: &Path, follow_symlinks: bool) -> Result<Gitignore, String> {
    let mut builder = GitignoreBuilder::new(root);
    match fs::symlink_metadata(file) {
        Ok(metadata) if follow_symlinks || !metadata.file_type().is_symlink() => {
            if let Some(error) = builder.add(file) {
                return Err(error.to_string());
            }
        }
        Ok(_) => {}
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
        let git = git_directory(&dot, metadata)?;
        let common = match fs::read_to_string(git.join("commondir")) {
            Ok(text) => git.join(text.trim()),
            Err(error) if error.kind() == io::ErrorKind::NotFound => git,
            Err(error) => return Err(error),
        };
        return Ok(Some((parent.to_path_buf(), common)));
    }
    Ok(None)
}

fn git_directory(dot: &Path, metadata: fs::Metadata) -> io::Result<PathBuf> {
    let metadata = if metadata.file_type().is_symlink() {
        fs::metadata(dot)?
    } else {
        metadata
    };
    if metadata.is_dir() {
        Ok(dot.to_path_buf())
    } else {
        gitdir(dot)
    }
}

fn gitdir(dot: &Path) -> io::Result<PathBuf> {
    let text = fs::read_to_string(dot)?;
    let relative = text
        .strip_prefix("gitdir:")
        .ok_or_else(|| io::Error::other("invalid .git file"))?
        .trim();
    Ok(dot.parent().unwrap_or(Path::new(".")).join(relative))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn malformed_nested_git_pointer_never_falls_back_to_an_ancestor_repository() {
        let root = tempfile::tempdir().unwrap();
        fs::create_dir(root.path().join(".git")).unwrap();
        let nested = root.path().join("nested");
        fs::create_dir(&nested).unwrap();
        assert_eq!(
            repository(&nested).unwrap(),
            Some((root.path().to_owned(), root.path().join(".git")))
        );
        fs::write(nested.join(".git"), "invalid pointer").unwrap();
        assert_eq!(
            repository(&nested).unwrap_err().to_string(),
            "invalid .git file"
        );
    }

    #[test]
    fn an_existing_invalid_commondir_is_an_error_rather_than_missing_metadata() {
        let root = tempfile::tempdir().unwrap();
        let git = root.path().join(".git");
        fs::create_dir(&git).unwrap();
        assert_eq!(
            repository(root.path()).unwrap(),
            Some((root.path().to_owned(), git.clone()))
        );
        fs::create_dir(git.join("commondir")).unwrap();
        assert!(repository(root.path()).is_err());
    }
}
