//! Native-driver marker matching follows fresh cells, not only output substrings.
#[path = "terminal_driver/screen.rs"]
mod screen;
use screen::Screen;

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
