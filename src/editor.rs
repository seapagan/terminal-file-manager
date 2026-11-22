//! tui-input owns single-line editing; field owners retain validation and approval.
use crossterm::event::{KeyCode, KeyEvent, KeyEventKind, KeyModifiers};
use tui_input::{Input, InputRequest};
use unicode_segmentation::UnicodeSegmentation;

#[derive(Clone, Debug, Default)]
pub struct Editor {
    input: Input,
    pub overwrite: bool,
}
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum Edit {
    Ignored,
    Cursor,
    Changed,
    Rejected,
}
impl Editor {
    pub fn position(&self, text: &str) -> usize {
        if self.input.value() != text {
            return text.len();
        }
        text.char_indices()
            .nth(self.input.cursor())
            .map_or(text.len(), |(i, _)| i)
    }
    fn synchronize(&mut self, text: &str) {
        if self.input.value() != text {
            self.input = Input::new(text.to_owned());
        }
    }
    pub fn key(&mut self, text: &mut String, key: KeyEvent, limit: usize) -> Edit {
        if key.kind == KeyEventKind::Release {
            return Edit::Cursor;
        }
        if key.modifiers.intersects(
            KeyModifiers::CONTROL
                | KeyModifiers::ALT
                | KeyModifiers::SUPER
                | KeyModifiers::META
                | KeyModifiers::HYPER,
        ) {
            return Edit::Ignored;
        }
        self.synchronize(text);
        if key.code == KeyCode::Insert && key.kind == KeyEventKind::Press {
            self.overwrite = !self.overwrite;
            return Edit::Cursor;
        }
        if let KeyCode::Char(c) = key.code
            && !c.is_control()
        {
            return self.insert(text, &c.to_string(), limit);
        }
        let Some(request) = request(key.code) else {
            return Edit::Ignored;
        };
        let response = self.input.handle(request);
        if response.is_some_and(|changed| changed.value) {
            *text = self.input.value().to_owned();
            Edit::Changed
        } else {
            Edit::Cursor
        }
    }
    /// Prepare a candidate so rejected edits cannot change the field or its cursor.
    pub fn insert(&mut self, text: &mut String, incoming: &str, limit: usize) -> Edit {
        self.synchronize(text);
        if incoming.is_empty() {
            return Edit::Cursor;
        }
        if incoming.len() > limit {
            return Edit::Rejected;
        }
        let start = self.position(text);
        let end = if self.overwrite {
            text[start..]
                .grapheme_indices(true)
                .take(incoming.graphemes(true).count())
                .last()
                .map_or(start, |(offset, grapheme)| start + offset + grapheme.len())
        } else {
            start
        };
        if text.len() - (end - start) + incoming.len() > limit {
            return Edit::Rejected;
        }
        let mut value = text.clone();
        value.replace_range(start..end, "");
        // Seed tui-input's bulk yank buffer instead of replaying every pasted character.
        let mut candidate = Input::new(incoming.to_owned());
        candidate.handle(InputRequest::DeleteLine);
        candidate = candidate.with_value(value).with_cursor(self.input.cursor());
        candidate.handle(InputRequest::Yank);
        // Combining insertion can join the next grapheme; settle using library movement.
        candidate.handle(InputRequest::GoToPrevChar);
        candidate.handle(InputRequest::GoToNextChar);
        *text = candidate.value().to_owned();
        self.input = candidate;
        Edit::Changed
    }
}

fn request(code: KeyCode) -> Option<InputRequest> {
    Some(match code {
        KeyCode::Left => InputRequest::GoToPrevChar,
        KeyCode::Right => InputRequest::GoToNextChar,
        KeyCode::Home => InputRequest::GoToStart,
        KeyCode::End => InputRequest::GoToEnd,
        KeyCode::Backspace => InputRequest::DeletePrevChar,
        KeyCode::Delete => InputRequest::DeleteNextChar,
        _ => return None,
    })
}

#[cfg(test)]
mod tests {
    use super::*;
    fn key(e: &mut Editor, text: &mut String, code: KeyCode) -> Edit {
        e.key(text, KeyEvent::new(code, KeyModifiers::NONE), 64)
    }
    #[test]
    fn cursor_editing_uses_graphemes_and_atomic_limits() {
        let mut e = Editor::default();
        let mut text = "a👩‍💻e\u{301}z".to_owned();
        key(&mut e, &mut text, KeyCode::Home);
        key(&mut e, &mut text, KeyCode::Right);
        key(&mut e, &mut text, KeyCode::Delete);
        assert_eq!(text, "ae\u{301}z");
        key(&mut e, &mut text, KeyCode::Right);
        key(&mut e, &mut text, KeyCode::Backspace);
        assert_eq!(text, "az");
        key(&mut e, &mut text, KeyCode::Insert);
        key(&mut e, &mut text, KeyCode::Char('界'));
        assert_eq!(text, "a界");
        assert_eq!(e.insert(&mut text, "too-long", 4), Edit::Rejected);
        assert_eq!(text, "a界");
        key(&mut e, &mut text, KeyCode::End);
        assert_eq!(e.position(&text), text.len());
    }
}

#[cfg(test)]
mod insertion_tests {
    use super::*;
    #[test]
    fn joined_graphemes_and_oversized_paste_keep_valid_library_cursor_state() {
        let mut editor = Editor::default();
        let mut text = "\u{301}z".to_owned();
        editor.key(
            &mut text,
            KeyEvent::new(KeyCode::Home, KeyModifiers::NONE),
            64,
        );
        assert_eq!(editor.insert(&mut text, "e", 64), Edit::Changed);
        assert_eq!(editor.position(&text), "e\u{301}".len());
        let before = editor.position(&text);
        assert_eq!(
            editor.insert(&mut text, &"x".repeat(65_536), 64),
            Edit::Rejected
        );
        assert_eq!(editor.position(&text), before);
        editor.key(
            &mut text,
            KeyEvent::new(KeyCode::Backspace, KeyModifiers::NONE),
            64,
        );
        assert_eq!(text, "z");
        assert_eq!(
            editor.insert(&mut text, &"x".repeat(65_535), 65_536),
            Edit::Changed
        );
        assert_eq!(text.len(), 65_536);
    }
}

#[cfg(test)]
mod overwrite_tests {
    use super::*;
    fn key(editor: &mut Editor, text: &mut String, code: KeyCode) {
        editor.key(text, KeyEvent::new(code, KeyModifiers::NONE), 65_536);
    }
    #[test]
    fn bulk_overwrite_preserves_graphemes_controls_and_unreplaced_text() {
        let mut editor = Editor::default();
        let mut text = "A👩‍💻e\u{301}\r\nZ".to_owned();
        key(&mut editor, &mut text, KeyCode::Home);
        key(&mut editor, &mut text, KeyCode::Right);
        key(&mut editor, &mut text, KeyCode::Insert);
        editor.insert(&mut text, "界Q", 64);
        assert_eq!(text, "A界Q\r\nZ");
        editor.insert(&mut text, "\r\n", 64);
        assert_eq!(text, "A界Q\r\nZ");
        assert_eq!(&text[editor.position(&text)..], "Z");
    }
    #[test]
    fn maximum_field_overwrite_uses_one_bounded_bulk_edit() {
        let mut editor = Editor::default();
        let mut text = "a".repeat(65_536);
        key(&mut editor, &mut text, KeyCode::Home);
        key(&mut editor, &mut text, KeyCode::Insert);
        let incoming = "b".repeat(65_536);
        assert_eq!(editor.insert(&mut text, &incoming, 65_536), Edit::Changed);
        assert_eq!(text, incoming);
        assert_eq!(editor.position(&text), text.len());
    }
}
