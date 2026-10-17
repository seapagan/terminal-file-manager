//! Unix formatters ported from lsplus (MIT, Grant Ramsay); see docs/provenance.md.
use crate::fs::Entry;

/// Unix permissions preference; Windows always retains native attributes.
#[derive(Clone, Copy, Debug, Default, Eq, PartialEq, serde::Serialize, serde::Deserialize)]
#[serde(rename_all = "kebab-case")]
pub enum PermissionDisplay {
    #[default]
    Symbolic,
    Octal,
}

/// Render native cached metadata, never treating missing mode bits as zero.
pub fn attributes(entry: &Entry, display: PermissionDisplay) -> String {
    if let Some(mode) = entry.mode {
        let kind = file_type(mode);
        match display {
            PermissionDisplay::Symbolic => format!("{kind}{}", mode_to_rwx(mode)),
            PermissionDisplay::Octal => format!("{kind} {}", mode_to_octal(mode)),
        }
    } else if entry.attributes.is_empty() {
        "unavailable".into()
    } else {
        entry.attributes.clone()
    }
}

// lsplus platform/unix.rs long_format_file_type + platform/mod.rs as_char.
fn file_type(mode: u32) -> char {
    match mode & 0o170000 {
        0o040000 => 'd',
        0o100000 => '-',
        0o120000 => 'l',
        0o140000 => 's',
        0o010000 => 'p',
        0o020000 => 'c',
        0o060000 => 'b',
        _ => '?',
    }
}

/// Convert Unix permission bits into an `rwxrwxrwx` string.
pub fn mode_to_rwx(mode: u32) -> String {
    let mut rwx = String::with_capacity(9);

    rwx.push(permission_char(mode, 0o400, 'r'));
    rwx.push(permission_char(mode, 0o200, 'w'));
    rwx.push(special_execute_char(mode, 0o4000, 0o100, 's', 'S'));

    rwx.push(permission_char(mode, 0o040, 'r'));
    rwx.push(permission_char(mode, 0o020, 'w'));
    rwx.push(special_execute_char(mode, 0o2000, 0o010, 's', 'S'));

    rwx.push(permission_char(mode, 0o004, 'r'));
    rwx.push(permission_char(mode, 0o002, 'w'));
    rwx.push(special_execute_char(mode, 0o1000, 0o001, 't', 'T'));

    rwx
}

/// Format Unix permission and special bits as four octal digits.
pub fn mode_to_octal(mode: u32) -> String {
    format!("{:04o}", mode & 0o7777)
}

fn permission_char(mode: u32, bit: u32, value: char) -> char {
    if mode & bit != 0 { value } else { '-' }
}

fn special_execute_char(
    mode: u32,
    special_bit: u32,
    execute_bit: u32,
    execute_char: char,
    no_execute_char: char,
) -> char {
    match (mode & special_bit != 0, mode & execute_bit != 0) {
        (true, true) => execute_char,
        (true, false) => no_execute_char,
        (false, true) => 'x',
        (false, false) => '-',
    }
}
