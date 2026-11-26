//! Native-driver marker matching follows fresh cells, not only output substrings.
#[path = "terminal_driver/screen.rs"]
mod screen;
use screen::Screen;

#[test]
fn wrapped_rows_retain_reused_letters_for_fresh_readiness_markers() {
    let mut screen = Screen::default();
    screen.observe(&vec![b' '; 300]).unwrap();
    screen.observe(b"No visible entries").unwrap();
    screen.clear_fresh();
    screen.observe(b"\x1b[4;9Hrowser-s\x1b[1Cntinel").unwrap();
    assert!(screen.contains_fresh(b"browser-sentinel"));
    screen.clear_fresh();
    assert!(!screen.contains_fresh(b"browser-sentinel"));
}

#[test]
fn resized_widths_and_wide_characters_keep_wrapped_coordinates() {
    let mut screen = Screen::default();
    screen.resize(4);
    screen.observe(b"abcd\x1b[31mef").unwrap();
    screen.clear_fresh();
    screen.observe(b"\x1b[2;2H!").unwrap();
    assert!(screen.contains_fresh(b"e!"));
    assert!(!screen.contains_fresh(b"abcd"));
    screen.resize(5);
    screen.observe("\x1b[3;5H界".as_bytes()).unwrap();
    screen.clear_fresh();
    screen.observe(b"\x1b[4;3HX").unwrap();
    assert!(screen.contains_fresh("界X".as_bytes()));
}

#[test]
fn shrinking_discards_marker_cells_beyond_the_right_edge() {
    let mut screen = Screen::default();
    screen.resize(8);
    screen.observe(b"ready").unwrap();
    screen.clear_fresh();
    screen.resize(4);
    screen.observe(b"\x1b[1;1Hr").unwrap();
    assert!(screen.contains_fresh(b"read"));
    assert!(!screen.contains_fresh(b"ready"));
    screen.resize(8);
    assert!(!screen.contains_fresh(b"ready"));
}

#[test]
fn cursor_motion_cancels_pending_wrap_at_the_last_column() {
    let mut screen = Screen::default();
    screen.resize(4);
    screen.observe(b"abcd").unwrap();
    screen.clear_fresh();
    screen.observe(b"\x1b[1DX\x1b[31mY\rz").unwrap();
    assert!(screen.contains_fresh(b"zbXY"));
}

#[test]
fn incremental_redraw_reuses_unchanged_cells_without_accepting_stale_rows() {
    let mut screen = Screen::default();
    screen
        .observe(b"\x1b[3;1HPattern selection\x1b[3;19H ")
        .unwrap();
    screen.clear_fresh();
    screen.observe(b"\x1b[4;1Hunrelated output").unwrap();
    assert!(!screen.contains_fresh(b"Patternselection"));
    screen.observe(b"\x1b[3;18H c").unwrap();
    screen.observe(b"\x1b[4;1Hinterleaved toast").unwrap();
    screen.observe(b"\x1b[3;20Hancelled").unwrap();
    assert!(screen.contains_fresh(b"Patternselectioncancelled"));
    screen.clear_fresh();
    assert!(!screen.contains_fresh(b"Patternselectioncancelled"));
}

#[test]
fn positioned_overwrites_and_erasure_remove_obsolete_markers() {
    let mut screen = Screen::default();
    screen.observe(b"\x1b[2;1Hcancelled").unwrap();
    screen.clear_fresh();
    screen.observe(b"\x1b[2;4H\x1b[31mX\x1b[0m").unwrap();
    assert!(!screen.contains_fresh(b"cancelled"));
    screen.observe(b"\x1b[2;4H\x1b[K").unwrap();
    assert!(!screen.contains_fresh(b"canXelled"));
    screen.observe(b"\x1b[2J").unwrap();
    assert!(!screen.contains_fresh(b"can"));
}

#[test]
fn split_sequences_and_wide_unicode_keep_ascii_marker_coordinates() {
    let mut screen = Screen::default();
    screen.observe(b"\x1b[2;").unwrap();
    screen.observe(b"1H\xe7\x95").unwrap();
    screen.observe(b"\x8c ready").unwrap();
    screen.clear_fresh();
    screen.observe(b"\x1b[2;4Hsteady").unwrap();
    assert!(screen.contains_fresh("界steady".as_bytes()));
    assert!(!screen.contains_fresh(b"ready"));
}
