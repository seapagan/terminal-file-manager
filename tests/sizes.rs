//! Original lsplus size regressions; MIT, Grant Ramsay.
use terminal_file_manager::size::{SizeScale, human_readable_format, show_size};

#[test]
fn test_format_size() {
    let (size, unit) = human_readable_format(0, SizeScale::Binary);
    assert_eq!(format!("{size:.1} {unit}"), "0.0 B");

    let (size, unit) = human_readable_format(1023, SizeScale::Binary);
    assert_eq!(format!("{size:.1} {unit}"), "1023.0 B");

    let (size, unit) = human_readable_format(1024, SizeScale::Binary);
    assert_eq!(format!("{size:.1} {unit}"), "1.0 K");

    let (size, unit) = human_readable_format(1024 * 1024, SizeScale::Binary);
    assert_eq!(format!("{size:.1} {unit}"), "1.0 M");

    let (size, unit) = human_readable_format(1024 * 1024 * 1024, SizeScale::Binary);
    assert_eq!(format!("{size:.1} {unit}"), "1.0 G");

    let (size, unit) = human_readable_format(1024 * 1024 * 1024 * 1024, SizeScale::Binary);
    assert_eq!(format!("{size:.1} {unit}"), "1.0 T");
}

#[test]
fn test_format_size_partial() {
    let (size, unit) = human_readable_format(1536, SizeScale::Binary);
    assert_eq!(format!("{size:.1} {unit}"), "1.5 K");

    let (size, unit) = human_readable_format(1024 * 1024 * 3 / 2, SizeScale::Binary);
    assert_eq!(format!("{size:.1} {unit}"), "1.5 M");

    let (size, unit) = human_readable_format(1024 * 1024 * 1024 * 5 / 2, SizeScale::Binary);
    assert_eq!(format!("{size:.1} {unit}"), "2.5 G");

    let (size, unit) = show_size(2560, Some(SizeScale::Binary));
    assert_eq!(size, "2.5");
    assert_eq!(unit, "K");

    let (size, unit) = show_size(1024, Some(SizeScale::Binary));
    assert_eq!(size, "1");
    assert_eq!(unit, "K");

    let (size, unit) = show_size(2560, None);
    assert_eq!(size, "2560");
    assert_eq!(unit, "");
}

#[test]
fn test_format_size_decimal() {
    let (size, unit) = human_readable_format(999, SizeScale::Decimal);
    assert_eq!(format!("{size:.1} {unit}"), "999.0 B");

    let (size, unit) = human_readable_format(1000, SizeScale::Decimal);
    assert_eq!(format!("{size:.1} {unit}"), "1.0 k");

    let (size, unit) = human_readable_format(1500, SizeScale::Decimal);
    assert_eq!(format!("{size:.1} {unit}"), "1.5 k");

    let (size, unit) = human_readable_format(1_000_000, SizeScale::Decimal);
    assert_eq!(format!("{size:.1} {unit}"), "1.0 M");

    let (size, unit) = show_size(1000, Some(SizeScale::Decimal));
    assert_eq!(size, "1");
    assert_eq!(unit, "k");
}

#[test]
fn test_format_size_extreme() {
    let (size, unit) = human_readable_format(1024 * 1024 * 1024 * 1024, SizeScale::Binary);
    assert_eq!(format!("{size:.1} {unit}"), "1.0 T");

    let (size, unit) = human_readable_format(1024 * 1024 * 1024 * 1024 * 1024, SizeScale::Binary);
    assert_eq!(format!("{size:.1} {unit}"), "1.0 P");

    let (size, unit) = human_readable_format(1024, SizeScale::Binary);
    assert_eq!(format!("{size:.1} {unit}"), "1.0 K");

    let (size, unit) = human_readable_format(1024 * 1024, SizeScale::Binary);
    assert_eq!(format!("{size:.1} {unit}"), "1.0 M");

    let (size, unit) = human_readable_format(1023, SizeScale::Binary);
    assert_eq!(format!("{size:.1} {unit}"), "1023.0 B");

    let (size, unit) = human_readable_format(1024 * 1024 - 1, SizeScale::Binary);
    assert_eq!(format!("{size:.1} {unit}"), "1.0 M");
}

#[test]
fn test_format_size_promotes_rounded_binary_boundaries() {
    let (size, unit) = show_size(1_048_524, Some(SizeScale::Binary));
    assert_eq!(size, "1023.9");
    assert_eq!(unit, "K");

    let (size, unit) = show_size(1_048_525, Some(SizeScale::Binary));
    assert_eq!(size, "1");
    assert_eq!(unit, "M");

    let (size, unit) = show_size(1_073_690_624, Some(SizeScale::Binary));
    assert_eq!(size, "1");
    assert_eq!(unit, "G");
}

#[test]
fn test_format_size_promotes_rounded_decimal_boundaries() {
    let (size, unit) = show_size(999_949, Some(SizeScale::Decimal));
    assert_eq!(size, "999.9");
    assert_eq!(unit, "k");

    let (size, unit) = show_size(999_950, Some(SizeScale::Decimal));
    assert_eq!(size, "1");
    assert_eq!(unit, "M");

    let (size, unit) = show_size(999_950_000, Some(SizeScale::Decimal));
    assert_eq!(size, "1");
    assert_eq!(unit, "G");
}
