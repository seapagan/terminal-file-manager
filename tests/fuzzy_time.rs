//! Original lsplus relative-time regressions, adapted to a fixed clock.
use std::time::{Duration, SystemTime};
use terminal_file_manager::fuzzy_time::fuzzy_time;

fn get_test_time(seconds_ago: u64) -> SystemTime {
    fixed_now()
        .checked_sub(Duration::from_secs(seconds_ago))
        .unwrap()
}

#[test]
fn test_fuzzy_time_seconds() {
    let time = get_test_time(30);
    assert_eq!(fuzzy_time(time, fixed_now()), "30 seconds ago");
}

#[test]
fn test_fuzzy_time_minutes() {
    let time = get_test_time(5 * 60);
    assert_eq!(fuzzy_time(time, fixed_now()), "5 minutes ago");

    let time = get_test_time(60);
    assert_eq!(fuzzy_time(time, fixed_now()), "1 minute ago");
}

#[test]
fn test_fuzzy_time_hours() {
    let time = get_test_time(2 * 60 * 60);
    assert_eq!(fuzzy_time(time, fixed_now()), "2 hours ago");

    let time = get_test_time(60 * 60);
    assert_eq!(fuzzy_time(time, fixed_now()), "1 hour ago");
}

#[test]
fn test_fuzzy_time_days() {
    let time = get_test_time(2 * 24 * 60 * 60);
    assert_eq!(fuzzy_time(time, fixed_now()), "2 days ago");

    let time = get_test_time(24 * 60 * 60);
    assert_eq!(fuzzy_time(time, fixed_now()), "yesterday");
}

#[test]
fn test_fuzzy_time_weeks() {
    let time = get_test_time(2 * 7 * 24 * 60 * 60);
    assert_eq!(fuzzy_time(time, fixed_now()), "2 weeks ago");

    let time = get_test_time(7 * 24 * 60 * 60);
    assert_eq!(fuzzy_time(time, fixed_now()), "last week");
}

#[test]
fn test_fuzzy_time_months() {
    let time = get_test_time(60 * 24 * 60 * 60);
    assert_eq!(fuzzy_time(time, fixed_now()), "2 months ago");

    let time = get_test_time(32 * 24 * 60 * 60);
    assert_eq!(fuzzy_time(time, fixed_now()), "last month");
}

#[test]
fn test_fuzzy_time_years() {
    let time = get_test_time(2 * 365 * 24 * 60 * 60);
    assert_eq!(fuzzy_time(time, fixed_now()), "2 years ago");

    let time = get_test_time(366 * 24 * 60 * 60);
    assert_eq!(fuzzy_time(time, fixed_now()), "last year");
}

#[test]
fn test_fuzzy_time_boundary_cases() {
    let now = fixed_now();

    let time = now.checked_sub(Duration::from_secs(59)).unwrap();
    assert_eq!(fuzzy_time(time, fixed_now()), "59 seconds ago");

    let time = now.checked_sub(Duration::from_secs(60)).unwrap();
    assert_eq!(fuzzy_time(time, fixed_now()), "1 minute ago");

    let time = now.checked_sub(Duration::from_secs(119)).unwrap();
    assert_eq!(fuzzy_time(time, fixed_now()), "1 minute ago");

    let time = now.checked_sub(Duration::from_secs(120)).unwrap();
    assert_eq!(fuzzy_time(time, fixed_now()), "2 minutes ago");

    let time = now.checked_sub(Duration::from_secs(23 * 60 * 60)).unwrap();
    assert_eq!(fuzzy_time(time, fixed_now()), "23 hours ago");

    let time = now.checked_sub(Duration::from_secs(24 * 60 * 60)).unwrap();
    assert_eq!(fuzzy_time(time, fixed_now()), "yesterday");

    let time = now
        .checked_sub(Duration::from_secs(6 * 24 * 60 * 60))
        .unwrap();
    assert_eq!(fuzzy_time(time, fixed_now()), "6 days ago");

    let time = now
        .checked_sub(Duration::from_secs(7 * 24 * 60 * 60))
        .unwrap();
    assert_eq!(fuzzy_time(time, fixed_now()), "last week");
}

#[test]
fn test_fuzzy_time_month_boundaries() {
    let now = fixed_now();

    let time = now
        .checked_sub(Duration::from_secs(29 * 24 * 60 * 60))
        .unwrap();
    assert_eq!(fuzzy_time(time, fixed_now()), "4 weeks ago");

    let time = now
        .checked_sub(Duration::from_secs(30 * 24 * 60 * 60))
        .unwrap();
    assert_eq!(fuzzy_time(time, fixed_now()), "last month");

    let time = now
        .checked_sub(Duration::from_secs(45 * 24 * 60 * 60))
        .unwrap();
    assert_eq!(fuzzy_time(time, fixed_now()), "last month");

    let time = now
        .checked_sub(Duration::from_secs(60 * 24 * 60 * 60))
        .unwrap();
    assert_eq!(fuzzy_time(time, fixed_now()), "2 months ago");
}

#[test]
fn test_fuzzy_time_year_boundaries() {
    let now = fixed_now();

    let time = now
        .checked_sub(Duration::from_secs(364 * 24 * 60 * 60))
        .unwrap();
    assert_eq!(fuzzy_time(time, fixed_now()), "12 months ago");

    let time = now
        .checked_sub(Duration::from_secs(365 * 24 * 60 * 60))
        .unwrap();
    assert_eq!(fuzzy_time(time, fixed_now()), "last year");

    let time = now
        .checked_sub(Duration::from_secs(729 * 24 * 60 * 60))
        .unwrap();
    assert_eq!(fuzzy_time(time, fixed_now()), "last year");

    let time = now
        .checked_sub(Duration::from_secs(730 * 24 * 60 * 60))
        .unwrap();
    assert_eq!(fuzzy_time(time, fixed_now()), "2 years ago");
}

#[test]
fn test_fuzzy_time_zero_duration() {
    let now = fixed_now();
    assert_eq!(fuzzy_time(now, now), "0 seconds ago");
}

#[test]
fn test_fuzzy_time_week_boundaries() {
    let now = fixed_now();

    let time = now
        .checked_sub(Duration::from_secs(13 * 24 * 60 * 60))
        .unwrap();
    assert_eq!(fuzzy_time(time, fixed_now()), "last week");

    let time = now
        .checked_sub(Duration::from_secs(14 * 24 * 60 * 60))
        .unwrap();
    assert_eq!(fuzzy_time(time, fixed_now()), "2 weeks ago");

    let time = now
        .checked_sub(Duration::from_secs(20 * 24 * 60 * 60))
        .unwrap();
    assert_eq!(fuzzy_time(time, fixed_now()), "2 weeks ago");

    let time = now
        .checked_sub(Duration::from_secs(21 * 24 * 60 * 60))
        .unwrap();
    assert_eq!(fuzzy_time(time, fixed_now()), "3 weeks ago");
}

#[test]
fn test_fuzzy_time_month_edge_cases() {
    let now = fixed_now();

    let time = now
        .checked_sub(Duration::from_secs(59 * 24 * 60 * 60))
        .unwrap();
    assert_eq!(fuzzy_time(time, fixed_now()), "last month");

    let time = now
        .checked_sub(Duration::from_secs(61 * 24 * 60 * 60))
        .unwrap();
    assert_eq!(fuzzy_time(time, fixed_now()), "2 months ago");

    let time = now
        .checked_sub(Duration::from_secs(89 * 24 * 60 * 60))
        .unwrap();
    assert_eq!(fuzzy_time(time, fixed_now()), "2 months ago");

    let time = now
        .checked_sub(Duration::from_secs(90 * 24 * 60 * 60))
        .unwrap();
    assert_eq!(fuzzy_time(time, fixed_now()), "3 months ago");
}

#[test]
fn test_fuzzy_time_very_old_files() {
    let now = fixed_now();

    let time = now
        .checked_sub(Duration::from_secs(2 * 365 * 24 * 60 * 60))
        .unwrap();
    assert_eq!(fuzzy_time(time, fixed_now()), "2 years ago");

    let time = now
        .checked_sub(Duration::from_secs(5 * 365 * 24 * 60 * 60))
        .unwrap();
    assert_eq!(fuzzy_time(time, fixed_now()), "5 years ago");

    let time = now
        .checked_sub(Duration::from_secs(10 * 365 * 24 * 60 * 60))
        .unwrap();
    assert_eq!(fuzzy_time(time, fixed_now()), "10 years ago");

    let time = now
        .checked_sub(Duration::from_secs(20 * 365 * 24 * 60 * 60))
        .unwrap();
    assert_eq!(fuzzy_time(time, fixed_now()), "20 years ago");

    let time = now
        .checked_sub(Duration::from_secs(50 * 365 * 24 * 60 * 60))
        .unwrap();
    assert_eq!(fuzzy_time(time, fixed_now()), "50 years ago");
}

#[test]
fn test_fuzzy_time_hour_edge_cases() {
    let now = fixed_now();

    let time = now.checked_sub(Duration::from_secs(59 * 60)).unwrap();
    assert_eq!(fuzzy_time(time, fixed_now()), "59 minutes ago");

    let time = now.checked_sub(Duration::from_secs(61 * 60)).unwrap();
    assert_eq!(fuzzy_time(time, fixed_now()), "1 hour ago");

    let time = now.checked_sub(Duration::from_secs(119 * 60)).unwrap();
    assert_eq!(fuzzy_time(time, fixed_now()), "1 hour ago");

    let time = now.checked_sub(Duration::from_secs(120 * 60)).unwrap();
    assert_eq!(fuzzy_time(time, fixed_now()), "2 hours ago");
}

fn get_future_test_time(seconds_from_now: u64) -> SystemTime {
    fixed_now()
        .checked_add(Duration::from_secs(seconds_from_now))
        .unwrap()
}

#[test]
fn test_fuzzy_time_future_times() {
    let time = get_future_test_time(90);
    assert_eq!(fuzzy_time(time, fixed_now()), "in 1 minute");

    let time = get_future_test_time(2 * 60 + 30);
    assert_eq!(fuzzy_time(time, fixed_now()), "in 2 minutes");

    let time = get_future_test_time(2 * 60 * 60 + 60);
    assert_eq!(fuzzy_time(time, fixed_now()), "in 2 hours");

    let time = get_future_test_time(14 * 24 * 60 * 60 + 60 * 60);
    assert_eq!(fuzzy_time(time, fixed_now()), "in 2 weeks");

    let time = get_future_test_time(2 * 365 * 24 * 60 * 60 + 24 * 60 * 60);
    assert_eq!(fuzzy_time(time, fixed_now()), "in 2 years");
}

fn fixed_now() -> SystemTime {
    std::time::UNIX_EPOCH + Duration::from_secs(2_000_000_000)
}

#[test]
fn exact_clock_boundaries_retain_original_past_and_future_semantics() {
    let now = fixed_now();
    for (seconds, before, at, future) in [
        (60, "59 seconds ago", "1 minute ago", "in 1 minute"),
        (3600, "59 minutes ago", "1 hour ago", "in 1 hour"),
        (86400, "23 hours ago", "yesterday", "in 1 day"),
        (604800, "6 days ago", "last week", "in 1 week"),
        (1209600, "last week", "2 weeks ago", "in 2 weeks"),
        (2592000, "4 weeks ago", "last month", "in 1 month"),
        (5184000, "last month", "2 months ago", "in 2 months"),
        (31536000, "12 months ago", "last year", "in 1 year"),
        (63072000, "last year", "2 years ago", "in 2 years"),
    ] {
        let boundary = Duration::from_secs(seconds);
        let tick = Duration::from_nanos(100); // native Windows FILETIME precision
        assert_eq!(fuzzy_time(now - boundary + tick, now), before);
        assert_eq!(fuzzy_time(now - boundary, now), at);
        assert_eq!(fuzzy_time(now - boundary - tick, now), at);
        assert_eq!(fuzzy_time(now + boundary, now), future);
    }
    assert_eq!(
        fuzzy_time(now + Duration::from_millis(500), now),
        "in 0 seconds"
    );
    assert_eq!(
        fuzzy_time(now - Duration::from_millis(500), now),
        "0 seconds ago"
    );
}
