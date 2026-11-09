use super::{Format, SourceIdentity};
use std::path::{Component, Path, PathBuf};

pub(super) const MAX_BYTES: usize = 16 * 1024 * 1024;

/// Lexical native relative paths; incomparable volumes never become absolute fallbacks.
pub fn relative(target: &Path, base: &Path) -> Result<PathBuf, String> {
    let target = components(target)?;
    let base = components(base)?;
    let common = target
        .iter()
        .zip(&base)
        .take_while(|(a, b)| equivalent(**a, **b))
        .count();
    if common == 0 || contains_root(&target[common..]) || contains_root(&base[common..]) {
        return Err("Relative paths require comparable native roots or volumes".into());
    }
    let mut result = PathBuf::new();
    for _ in &base[common..] {
        result.push("..");
    }
    for part in &target[common..] {
        result.push(part.as_os_str());
    }
    if result.as_os_str().is_empty() {
        result.push(".");
    }
    Ok(result)
}

fn contains_root(parts: &[Component<'_>]) -> bool {
    parts
        .iter()
        .any(|c| matches!(c, Component::Prefix(_) | Component::RootDir))
}

fn components(path: &Path) -> Result<Vec<Component<'_>>, String> {
    if !path.is_absolute() {
        return Err("Clipboard paths require an absolute local reference".into());
    }
    let mut result = Vec::new();
    for part in path.components() {
        match part {
            Component::CurDir => {}
            Component::ParentDir if matches!(result.last(), Some(Component::Normal(_))) => {
                result.pop();
            }
            Component::ParentDir => {}
            _ => result.push(part),
        }
    }
    Ok(result)
}

fn equivalent(a: Component<'_>, b: Component<'_>) -> bool {
    #[cfg(windows)]
    if matches!((a, b), (Component::Prefix(_), Component::Prefix(_))) {
        return a
            .as_os_str()
            .as_encoded_bytes()
            .eq_ignore_ascii_case(b.as_os_str().as_encoded_bytes());
    }
    a == b
}

pub(super) fn text(
    identity: &SourceIdentity,
    format: Format,
    base: Option<&Path>,
) -> Result<String, String> {
    let SourceIdentity::Local(entry) = identity;
    let path = match format {
        Format::Name => PathBuf::from(entry.path.file_name().unwrap_or(entry.path.as_os_str())),
        Format::Full => entry.path.clone(),
        Format::Relative => relative(
            &entry.path,
            base.ok_or("Relative reference is unavailable")?,
        )?,
    };
    let text = path
        .to_str()
        .ok_or("Path is not valid Unicode; a text clipboard cannot copy it losslessly")?;
    if text.is_empty()
        || text
            .chars()
            .any(|c| c.is_control() || matches!(c, '\u{2028}' | '\u{2029}'))
    {
        return Err("Path contains a control character or line break; copy a different text representation outside TFM".into());
    }
    Ok(text.into())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[cfg(unix)]
    #[test]
    fn non_unicode_native_spelling_is_rejected_in_every_text_format() {
        use std::os::unix::ffi::OsStringExt;
        let root = tempfile::tempdir().unwrap();
        let mut entry = crate::fs::inspect(root.path());
        entry.path = root
            .path()
            .join(std::ffi::OsString::from_vec(vec![b'x', 0xff]));
        let source = SourceIdentity::Local(entry);
        for format in [Format::Name, Format::Relative, Format::Full] {
            assert!(
                text(&source, format, Some(root.path()))
                    .unwrap_err()
                    .contains("Unicode")
            );
        }
    }

    #[test]
    fn filesystem_roots_have_nonempty_native_name_text() {
        let root = tempfile::tempdir().unwrap();
        let mut entry = crate::fs::inspect(root.path());
        for path in if cfg!(windows) {
            vec![r"C:\", r"\\server\share\", r"\\?\C:\"]
        } else {
            vec!["/"]
        } {
            entry.path = path.into();
            let output = text(&SourceIdentity::Local(entry.clone()), Format::Name, None).unwrap();
            assert_eq!(output, path);
        }
    }
}
