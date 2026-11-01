//! Ported from lsplus (MIT, Grant Ramsay); see docs/provenance.md.
//! Human-readable relative timestamp formatting.
//!
//! Past timestamps are rendered as phrases such as `2 hours ago`, `yesterday`,
//! or `last month`; future timestamps are rendered as `in ...` phrases.

use std::fmt;
use std::time::{Duration, SystemTime};

/// One calendar-ish day.
pub(crate) const DAY: Duration = Duration::from_secs(24 * 60 * 60);
/// Seven days.
pub(crate) const WEEK: Duration = Duration::from_secs(7 * DAY.as_secs());
/// Thirty days, used as the month bucket for display gradients.
pub(crate) const MONTH: Duration = Duration::from_secs(30 * DAY.as_secs());
/// Three hundred sixty-five days.
pub(crate) const YEAR: Duration = Duration::from_secs(365 * DAY.as_secs());

#[derive(Debug)]
enum FuzzyTime {
    Ago(&'static str, u64),
    LastWeek,
    LastMonth,
    LastYear,
    Yesterday,
}

/// Format one relative-time unit with simple pluralization.
fn format_unit(unit: &str, n: u64) -> String {
    format!("{} {}{}", n, unit, if n == 1 { "" } else { "s" })
}

impl fmt::Display for FuzzyTime {
    fn fmt(&self, f: &mut fmt::Formatter) -> fmt::Result {
        match self {
            FuzzyTime::Ago(unit, n) => write!(f, "{} ago", format_unit(unit, *n)),
            FuzzyTime::LastWeek => write!(f, "last week"),
            FuzzyTime::LastMonth => write!(f, "last month"),
            FuzzyTime::LastYear => write!(f, "last year"),
            FuzzyTime::Yesterday => write!(f, "yesterday"),
        }
    }
}

/// Format a future timestamp offset.
fn format_future_time(duration: Duration) -> String {
    let (unit, n) = if duration < DAY {
        subday_unit(duration.as_secs())
    } else {
        calendar_unit(duration)
    };
    format!("in {}", format_unit(unit, n))
}

fn subday_unit(seconds: u64) -> (&'static str, u64) {
    if seconds < 60 {
        ("second", seconds)
    } else if seconds < 3600 {
        ("minute", seconds / 60)
    } else {
        ("hour", seconds / 3600)
    }
}

fn calendar_unit(duration: Duration) -> (&'static str, u64) {
    let (unit, bucket) = if duration < WEEK {
        ("day", DAY)
    } else if duration < MONTH {
        ("week", WEEK)
    } else if duration < YEAR {
        ("month", MONTH)
    } else {
        ("year", YEAR)
    };
    (unit, duration.as_secs() / bucket.as_secs())
}

/// Bucket a past timestamp offset into the phrase used for display.
fn get_fuzzy_time(duration: Duration) -> FuzzyTime {
    let seconds = duration.as_secs();
    let minutes = seconds / 60;
    let hours = minutes / 60;
    match seconds {
        s if s < 60 => FuzzyTime::Ago("second", s),
        _ if minutes < 60 => FuzzyTime::Ago("minute", minutes),
        _ if hours < 24 => FuzzyTime::Ago("hour", hours),
        _ => older_fuzzy_time(duration),
    }
}

fn older_fuzzy_time(duration: Duration) -> FuzzyTime {
    if duration < MONTH {
        return recent_fuzzy_time(duration);
    }
    if duration < 2 * MONTH {
        FuzzyTime::LastMonth
    } else if duration < YEAR {
        FuzzyTime::Ago("month", duration.as_secs() / MONTH.as_secs())
    } else if duration < 2 * YEAR {
        FuzzyTime::LastYear
    } else {
        FuzzyTime::Ago("year", duration.as_secs() / YEAR.as_secs())
    }
}

fn recent_fuzzy_time(duration: Duration) -> FuzzyTime {
    let days = duration.as_secs() / DAY.as_secs();
    if days == 1 {
        FuzzyTime::Yesterday
    } else if duration < WEEK {
        FuzzyTime::Ago("day", days)
    } else if duration < 2 * WEEK {
        FuzzyTime::LastWeek
    } else {
        FuzzyTime::Ago("week", duration.as_secs() / WEEK.as_secs())
    }
}

/// Format a timestamp against one explicit clock, preserving lsplus duration buckets.
/// No local/UTC conversion is needed for relative durations.
pub fn fuzzy_time(time: SystemTime, now: SystemTime) -> String {
    match now.duration_since(time) {
        Ok(duration) => get_fuzzy_time(duration).to_string(),
        Err(error) => format_future_time(error.duration()),
    }
}
