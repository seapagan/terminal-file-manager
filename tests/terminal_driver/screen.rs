//! Track positioned text emitted by the native terminal, including partial redraws.
use std::collections::BTreeMap;
use std::io;

pub struct Screen {
    cells: BTreeMap<(usize, usize), (String, bool)>,
    position: (usize, usize),
    escape: Vec<u8>,
    utf8: Vec<u8>,
    columns: usize,
}

impl Default for Screen {
    fn default() -> Self {
        Self {
            cells: BTreeMap::new(),
            position: (0, 0),
            escape: Vec::new(),
            utf8: Vec::new(),
            columns: 100,
        }
    }
}

impl Screen {
    pub fn resize(&mut self, columns: usize) {
        self.columns = columns.max(1);
        self.position.1 = self.position.1.min(self.columns - 1);
    }

    pub fn observe(&mut self, bytes: &[u8]) -> io::Result<()> {
        for &byte in bytes {
            if !self.escape.is_empty() {
                self.escape.push(byte);
                if self.escape.len() == 2 && byte != b'[' {
                    self.escape.clear();
                } else if self.escape.len() > 2 && (0x40..=0x7e).contains(&byte) {
                    self.control();
                    self.escape.clear();
                }
            } else {
                self.byte(byte);
            }
            if self.escape.len() > 128 || self.utf8.len() > 4 || self.cells.len() > 65536 {
                return Err(io::Error::other(
                    "terminal screen buffer exceeded its bound",
                ));
            }
        }
        Ok(())
    }

    pub fn clear_fresh(&mut self) {
        for (_, fresh) in self.cells.values_mut() {
            *fresh = false;
        }
    }

    pub fn contains_fresh(&self, marker: &[u8]) -> bool {
        let cells: Vec<_> = self
            .cells
            .values()
            .flat_map(|(text, fresh)| {
                text.bytes()
                    .filter(|b| !b.is_ascii_whitespace())
                    .map(|byte| (byte, *fresh))
            })
            .collect();
        cells.windows(marker.len()).any(|window| {
            window
                .iter()
                .map(|(byte, _)| *byte)
                .eq(marker.iter().copied())
                && window.iter().any(|(_, fresh)| *fresh)
        })
    }

    fn byte(&mut self, byte: u8) {
        match byte {
            0x1b => self.escape.push(byte),
            b'\r' => self.position.1 = 0,
            b'\n' => {
                self.position.0 = self.position.0.saturating_add(1);
                self.position.1 = self.position.1.min(self.columns - 1);
            }
            8 => self.position.1 = self.position.1.min(self.columns - 1).saturating_sub(1),
            0..=31 | 127 => {}
            _ => {
                self.utf8.push(byte);
                if let Ok(text) = std::str::from_utf8(&self.utf8) {
                    let text = text.to_owned();
                    self.write(text);
                    self.utf8.clear();
                }
            }
        }
    }

    fn write(&mut self, text: String) {
        let width = ratatui::text::Line::from(text.as_str()).width();
        if width == 0 {
            if let Some((_, (previous, fresh))) = self.cells.range_mut(..self.position).next_back()
            {
                previous.push_str(&text);
                *fresh = true;
            }
            return;
        }
        if self.position.1.saturating_add(width) > self.columns {
            self.position = (self.position.0.saturating_add(1), 0);
        }
        self.cells.insert(self.position, (text, true));
        for offset in 1..width {
            self.cells.insert(
                (self.position.0, self.position.1.saturating_add(offset)),
                (String::new(), true),
            );
        }
        self.position.1 = self.position.1.saturating_add(width);
    }

    fn control(&mut self) {
        let command = *self.escape.last().unwrap();
        let moves = matches!(
            command,
            b'H' | b'f' | b'G' | b'd' | b'A' | b'B' | b'C' | b'D' | b'J' | b'K'
        );
        if moves {
            self.position.1 = self.position.1.min(self.columns - 1);
        }
        let arguments = String::from_utf8_lossy(&self.escape[2..self.escape.len() - 1]);
        let values: Vec<usize> = arguments
            .split(';')
            .map(|v| v.parse().unwrap_or(0))
            .collect();
        let first = values.first().copied().unwrap_or(0);
        let amount = first.max(1);
        match command {
            b'H' | b'f' => {
                self.position = (amount - 1, values.get(1).copied().unwrap_or(1).max(1) - 1)
            }
            b'G' => self.position.1 = amount - 1,
            b'd' => self.position.0 = amount - 1,
            b'A' => self.position.0 = self.position.0.saturating_sub(amount),
            b'B' => self.position.0 = self.position.0.saturating_add(amount),
            b'C' => self.position.1 = self.position.1.saturating_add(amount),
            b'D' => self.position.1 = self.position.1.saturating_sub(amount),
            b'J' => self.erase_display(first),
            b'K' => self.erase_line(first),
            _ => {}
        }
        if moves {
            self.position.1 = self.position.1.min(self.columns - 1);
        }
    }

    fn erase_display(&mut self, mode: usize) {
        let cursor = self.position;
        self.cells.retain(|&position, _| match mode {
            0 => position < cursor,
            1 => position > cursor,
            _ => false,
        });
    }

    fn erase_line(&mut self, mode: usize) {
        let (row, column) = self.position;
        self.cells.retain(|&(y, x), _| {
            y != row
                || match mode {
                    0 => x < column,
                    1 => x > column,
                    _ => false,
                }
        });
    }
}
