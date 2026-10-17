//! Size formatting ported from lsplus (MIT, Grant Ramsay); see docs/provenance.md.
/// Unit scaling mode for human-readable file sizes.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum SizeScale {
    /// Scale by powers of 1024 and use GNU-style binary suffixes.
    Binary,
    /// Scale by powers of 1000 and use GNU-style decimal suffixes.
    Decimal,
}

impl SizeScale {
    fn base(self) -> f64 {
        match self {
            Self::Binary => 1024.0,
            Self::Decimal => 1000.0,
        }
    }

    fn units(self) -> [&'static str; 6] {
        match self {
            Self::Binary => ["B", "K", "M", "G", "T", "P"],
            Self::Decimal => ["B", "k", "M", "G", "T", "P"],
        }
    }
}

/// Scale a byte count into a human-readable value and unit.
pub fn human_readable_format(size: u64, scale: SizeScale) -> (f64, &'static str) {
    let units = scale.units();
    let mut size = size as f64;
    let mut unit_index = 0;

    while size >= scale.base() && unit_index < units.len() - 1 {
        size /= scale.base();
        unit_index += 1;
    }

    let rounded_size = round_to_display_precision(size);
    if rounded_size >= scale.base() && unit_index < units.len() - 1 {
        size = rounded_size / scale.base();
        unit_index += 1;
    }

    (size, units[unit_index])
}

fn round_to_display_precision(size: f64) -> f64 {
    (size * 10.0).round() / 10.0
}

/// Format a size for display and return the optional unit label.
pub fn show_size(size: u64, scale: Option<SizeScale>) -> (String, &'static str) {
    let Some(scale) = scale else {
        return (size.to_string(), "");
    };

    let (size, unit) = human_readable_format(size, scale);
    if size.fract() == 0.0 {
        (format!("{size:.0}"), unit)
    } else {
        (format!("{size:.1}"), unit)
    }
}
