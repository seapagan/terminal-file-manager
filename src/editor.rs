//! Single-line cursor state; field owners retain text, limits and approval semantics.
use crossterm::event::{KeyCode, KeyEvent, KeyEventKind, KeyModifiers};
use ratatui::{style::Style, text::Line};

#[derive(Clone, Copy, Debug)]
pub struct Editor {
    cursor: usize,
    pub overwrite: bool,
}
impl Default for Editor {
    fn default() -> Self {
        Self {
            cursor: usize::MAX,
            overwrite: false,
        }
    }
}
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum Edit {
    Ignored,
    Cursor,
    Changed,
    Rejected,
}

/// Grapheme boundaries come from the existing renderer; retain skipped control bytes too.
pub(crate) fn boundaries(text: &str) -> Vec<usize> {
    let mut result = vec![0];
    let mut offset = 0;
    for g in Line::raw(text).styled_graphemes(Style::default()) {
        let start = offset + text[offset..].find(g.symbol).unwrap_or(0);
        result.extend(
            text[offset..start]
                .char_indices()
                .map(|(i, c)| offset + i + c.len_utf8()),
        );
        offset = start + g.symbol.len();
        result.push(offset);
    }
    result.extend(
        text[offset..]
            .char_indices()
            .map(|(i, c)| offset + i + c.len_utf8()),
    );
    result
}

impl Editor {
    pub fn position(&self, text: &str) -> usize {
        boundaries(text)
            .into_iter()
            .take_while(|p| *p <= self.cursor.min(text.len()))
            .last()
            .unwrap_or(0)
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
        self.cursor = self.position(text);
        if self.move_cursor(text, key) {
            return Edit::Cursor;
        }
        match key.code {
            KeyCode::Backspace => self.erase(text, true),
            KeyCode::Delete => self.erase(text, false),
            KeyCode::Char(c) if !c.is_control() => self.insert(text, &c.to_string(), limit),
            _ => Edit::Ignored,
        }
    }
    fn move_cursor(&mut self, text: &str, key: KeyEvent) -> bool {
        match key.code {
            KeyCode::Left => {
                self.cursor = boundaries(text)
                    .into_iter()
                    .take_while(|p| *p < self.cursor)
                    .last()
                    .unwrap_or(0)
            }
            KeyCode::Right => self.cursor = self.next(text),
            KeyCode::Home => self.cursor = 0,
            KeyCode::End => self.cursor = text.len(),
            KeyCode::Insert if key.kind == KeyEventKind::Press => self.overwrite = !self.overwrite,
            _ => return false,
        }
        true
    }

    fn next(&self, text: &str) -> usize {
        boundaries(text)
            .into_iter()
            .find(|p| *p > self.cursor)
            .unwrap_or(text.len())
    }
    fn erase(&mut self, text: &mut String, backwards: bool) -> Edit {
        let start = if backwards {
            boundaries(text)
                .into_iter()
                .take_while(|p| *p < self.cursor)
                .last()
                .unwrap_or(0)
        } else {
            self.cursor
        };
        let end = if backwards {
            self.cursor
        } else {
            self.next(text)
        };
        if start == end {
            return Edit::Cursor;
        }
        text.replace_range(start..end, "");
        self.cursor = start;
        self.cursor = self.position(text);
        Edit::Changed
    }
    /// Reject an oversized edit atomically; callers choose sanitization and validation.
    pub fn insert(&mut self, text: &mut String, incoming: &str, limit: usize) -> Edit {
        self.cursor = self.position(text);
        let mut end = self.cursor;
        if self.overwrite {
            let count = boundaries(incoming).len().saturating_sub(1);
            end = boundaries(text)
                .into_iter()
                .filter(|p| *p >= self.cursor)
                .nth(count)
                .unwrap_or(text.len());
        }
        if text.len() - (end - self.cursor) + incoming.len() > limit {
            return Edit::Rejected;
        }
        if incoming.is_empty() {
            return Edit::Cursor;
        }
        text.replace_range(self.cursor..end, incoming);
        self.cursor += incoming.len();
        // An inserted combining sequence can join the following grapheme.
        self.cursor = boundaries(text)
            .into_iter()
            .find(|p| *p >= self.cursor)
            .unwrap_or(text.len());
        Edit::Changed
    }
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
