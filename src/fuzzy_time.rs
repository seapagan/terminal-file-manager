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
    Ago(TimeUnit, u64),
    LastWeek,
    LastMonth,
    LastYear,
    Yesterday,
}

#[derive(Debug)]
enum TimeUnit {
    Second,
    Minute,
    Hour,
    Day,
    Week,
    Month,
    Year,
}

impl TimeUnit {
    fn as_str(&self) -> &'static str {
        match self {
            Self::Second => "second",
            Self::Minute => "minute",
            Self::Hour => "hour",
            Self::Day => "day",
            Self::Week => "week",
            Self::Month => "month",
            Self::Year => "year",
        }
    }
}

/// Format one relative-time unit with simple pluralization.
fn format_unit(unit: &TimeUnit, n: u64) -> String {
    format!("{} {}{}", n, unit.as_str(), if n == 1 { "" } else { "s" })
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
    let (unit, n) = duration_unit(duration);
    format!("in {}", format_unit(&unit, n))
}

fn duration_unit(duration: Duration) -> (TimeUnit, u64) {
    if duration < DAY {
        subday_unit(duration.as_secs())
    } else {
        calendar_unit(duration)
    }
}

fn subday_unit(seconds: u64) -> (TimeUnit, u64) {
    if seconds < 60 {
        (TimeUnit::Second, seconds)
    } else if seconds < 3600 {
        (TimeUnit::Minute, seconds / 60)
    } else {
        (TimeUnit::Hour, seconds / 3600)
    }
}

fn calendar_unit(duration: Duration) -> (TimeUnit, u64) {
    let (unit, bucket) = if duration < WEEK {
        (TimeUnit::Day, DAY)
    } else if duration < MONTH {
        (TimeUnit::Week, WEEK)
    } else if duration < YEAR {
        (TimeUnit::Month, MONTH)
    } else {
        (TimeUnit::Year, YEAR)
    };
    (unit, duration.as_secs() / bucket.as_secs())
}

/// Bucket a past timestamp offset into the phrase used for display.
fn get_fuzzy_time(duration: Duration) -> FuzzyTime {
    match duration_unit(duration) {
        (TimeUnit::Day, 1) => FuzzyTime::Yesterday,
        (TimeUnit::Week, 1) => FuzzyTime::LastWeek,
        (TimeUnit::Month, 1) => FuzzyTime::LastMonth,
        (TimeUnit::Year, 1) => FuzzyTime::LastYear,
        (unit, n) => FuzzyTime::Ago(unit, n),
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
