//! lsplus symbolic/octal fixtures and TFM native metadata contracts.
use terminal_file_manager::{
    fs,
    permissions::{PermissionDisplay, attributes, mode_to_octal, mode_to_rwx},
};

#[test]
fn unix_modes_preserve_special_bits_and_file_types() {
    let root = tempfile::tempdir().unwrap();
    for (mode, symbolic, octal) in [
        (0o100644, "-rw-r--r--", "0644"),
        (0o040755, "drwxr-xr-x", "0755"),
        (0o120777, "lrwxrwxrwx", "0777"),
        (0o104755, "-rwsr-xr-x", "4755"),
        (0o104644, "-rwSr--r--", "4644"),
        (0o102755, "-rwxr-sr-x", "2755"),
        (0o102644, "-rw-r-Sr--", "2644"),
        (0o041755, "drwxr-xr-t", "1755"),
        (0o041644, "drw-r--r-T", "1644"),
        (0o107777, "-rwsrwsrwt", "7777"),
    ] {
        assert_eq!(mode_to_rwx(mode), &symbolic[1..]);
        assert_eq!(mode_to_octal(mode), octal);
        let mut entry = fs::inspect(&root.path().join("missing"));
        entry.mode = Some(mode);
        assert_eq!(attributes(&entry, PermissionDisplay::Symbolic), symbolic);
        assert_eq!(
            attributes(&entry, PermissionDisplay::Octal),
            format!("{} {octal}", &symbolic[..1])
        );
    }
}

#[test]
fn unavailable_modes_never_invent_unix_permissions() {
    let root = tempfile::tempdir().unwrap();
    let entry = fs::inspect(&root.path().join("missing"));
    assert_eq!(
        attributes(&entry, PermissionDisplay::Symbolic),
        "unavailable"
    );
    assert_eq!(attributes(&entry, PermissionDisplay::Octal), "unavailable");
}

#[cfg(unix)]
#[test]
fn native_symlink_modes_describe_the_link_itself() {
    use std::os::unix::fs::{PermissionsExt, symlink};
    let root = tempfile::tempdir().unwrap();
    let file = root.path().join("file");
    let link = root.path().join("link");
    std::fs::write(&file, "data").unwrap();
    std::fs::set_permissions(&file, std::fs::Permissions::from_mode(0o4644)).unwrap();
    symlink(&file, &link).unwrap();
    assert_eq!(
        attributes(&fs::inspect(&file), PermissionDisplay::Symbolic),
        "-rwSr--r--"
    );
    assert!(attributes(&fs::inspect(&link), PermissionDisplay::Symbolic).starts_with('l'));
    std::fs::remove_file(file).unwrap();
    assert!(attributes(&fs::inspect(&link), PermissionDisplay::Symbolic).starts_with('l'));
}

#[cfg(windows)]
#[test]
fn windows_native_attributes_survive_both_permission_preferences() {
    let root = tempfile::tempdir().unwrap();
    let entry = fs::inspect(root.path());
    assert!(entry.mode.is_none());
    assert_eq!(
        attributes(&entry, PermissionDisplay::Symbolic),
        entry.attributes
    );
    assert_eq!(
        attributes(&entry, PermissionDisplay::Octal),
        entry.attributes
    );
}

#[test]
fn test_format_mode() {
    assert_eq!(mode_to_rwx(0o755), "rwxr-xr-x");
    assert_eq!(mode_to_rwx(0o644), "rw-r--r--");
    assert_eq!(mode_to_rwx(0o777), "rwxrwxrwx");
}

#[test]
fn test_format_mode_permissions() {
    assert_eq!(mode_to_rwx(0o000), "---------");
    assert_eq!(mode_to_rwx(0o777), "rwxrwxrwx");
    assert_eq!(mode_to_rwx(0o750), "rwxr-x---");
}

#[test]
fn test_mode_to_rwx_edge_cases() {
    assert_eq!(mode_to_rwx(0o0000), "---------");
    assert_eq!(mode_to_rwx(0o0777), "rwxrwxrwx");
    assert_eq!(mode_to_rwx(0o0222), "-w--w--w-");
    assert_eq!(mode_to_rwx(0o0111), "--x--x--x");
}

#[test]
fn test_mode_to_rwx_special_bits() {
    assert_eq!(mode_to_rwx(0o4755), "rwsr-xr-x");
    assert_eq!(mode_to_rwx(0o4644), "rwSr--r--");
    assert_eq!(mode_to_rwx(0o2755), "rwxr-sr-x");
    assert_eq!(mode_to_rwx(0o2644), "rw-r-Sr--");
    assert_eq!(mode_to_rwx(0o1755), "rwxr-xr-t");
    assert_eq!(mode_to_rwx(0o1644), "rw-r--r-T");
    assert_eq!(mode_to_rwx(0o7777), "rwsrwsrwt");
}

#[test]
fn test_mode_to_octal_masks_permission_bits() {
    assert_eq!(mode_to_octal(0o100644), "0644");
    assert_eq!(mode_to_octal(0o100755), "0755");
    assert_eq!(mode_to_octal(0o0000), "0000");
    assert_eq!(mode_to_octal(0o104755), "4755");
    assert_eq!(mode_to_octal(0o041777), "1777");
    assert_eq!(mode_to_octal(0o107777), "7777");
}
