use super::*;

#[test]
fn safety_hints_use_the_same_evidence_key_as_controls() {
    let (_root, mut app) = fixture();
    for index in 0..app.operations.history.len() {
        app.operations.cursor = index;
        let (terminal, _) = draw(&app, (120, 40), ColorLevel::NoColor);
        let text: String = terminal
            .backend()
            .buffer()
            .content()
            .iter()
            .map(|cell| cell.symbol())
            .collect();
        assert!(!text.contains("E evidence"));
        assert!(!text.contains("no R/exit"));
        assert!(text.contains("e Evidence"));
    }
}
