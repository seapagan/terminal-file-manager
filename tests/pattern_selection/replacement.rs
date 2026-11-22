use super::*;

fn marked_panes() -> (tempfile::TempDir, App) {
    let (root, mut app) = loaded();
    app.complete_listing(
        1,
        1,
        Ok(list(root.path(), &IgnoreContext::isolated(), &false.into()).unwrap()),
    );
    for pane in &mut app.panes {
        pane.show_hidden = true;
        pane.refilter();
        pane.select_all();
    }
    (root, app)
}

#[test]
fn replace_clears_all_active_pane_marks_then_marks_only_visible_matches() {
    for active in 0..2 {
        let (root, mut app) = marked_panes();
        let other = app.panes[1 - active].marks.clone();
        let pane = &mut app.panes[active];
        pane.show_hidden = false;
        pane.set_filter("txt".into());
        let mut selection = PatternSelection::new(pane, active);
        selection.set_expression("*.txt".into());
        let preview = selection.preview(pane).unwrap();
        assert_eq!(
            (preview.visible, preview.marked, preview.matches),
            (4, 6, 3)
        );
        selection.apply(pane, PatternOperation::Replace).unwrap();
        assert_eq!(pane.marks.len(), 3);
        for name in ["alpha.txt", "λ.txt", "folder.txt"] {
            assert!(pane.marks.contains_key(&root.path().join(name)));
        }
        assert_eq!(app.panes[1 - active].marks, other);
    }
}

#[test]
fn zero_match_replace_clears_all_marks_even_with_an_empty_visible_listing() {
    for active in 0..2 {
        for filter in ["txt", "no-visible-entries"] {
            let (_root, mut app) = marked_panes();
            let other = app.panes[1 - active].marks.clone();
            let pane = &mut app.panes[active];
            pane.show_hidden = false;
            pane.set_filter(filter.into());
            let mut selection = PatternSelection::new(pane, active);
            selection.set_expression("missing".into());
            assert_eq!(selection.preview(pane).unwrap().marked, 6);
            assert_eq!(
                selection
                    .apply(pane, PatternOperation::Replace)
                    .unwrap()
                    .matches,
                0
            );
            assert!(pane.marks.is_empty());
            assert_eq!(app.panes[1 - active].marks, other);
        }
    }
}
