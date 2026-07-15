//! A shared, editable single- or multi-line text buffer with a movable caret.
//!
//! [`TextInput`] is the one place the fiddly char-boundary and multiline scroll math lives, so
//! every form field, dialog field, and detail-view edit buffer adopts it instead of hand-rolling
//! an append-only `push`/`pop` pair. The caret is held as a **char index** (not a byte offset), so
//! every mutation and movement is UTF-8-safe and never indexes into the middle of a multi-byte
//! character. It holds only transient process-lifetime UI state (hard-constraint #1): the buffer,
//! its caret, and — derived on demand for rendering — the scroll offset; nothing is persisted.
//!
//! Two render helpers turn the buffer + caret into what the draw layer needs:
//!
//! - [`TextInput::field_view`] for a single-line field: the visible substring (horizontally
//!   scrolled so the caret stays in view) plus the caret's column.
//! - [`TextInput::viewport`] for a multiline pane: the visible, hard-wrapped rows for a
//!   `width × height` viewport (vertically scrolled to keep the caret line visible) plus the
//!   caret's `(row, col)` within that viewport.

#[cfg(test)]
mod tests;

/// A rendered slice of a [`TextInput`] for a multiline viewport: the visible rows and the caret's
/// position within them.
///
/// The rows are hard-wrapped at the viewport width (honouring embedded `'\n'`) and the caret's
/// `(row, col)` are relative to the viewport's top-left, so the draw layer can render `lines`
/// verbatim and place the terminal cursor at `(caret_row, caret_col)`.
#[derive(Debug, Clone, PartialEq, Eq, Default)]
pub struct Viewport {
    /// The visible visual rows, top to bottom (at most the requested height).
    pub lines: Vec<String>,
    /// The caret's row within the viewport (`0..height`).
    pub caret_row: u16,
    /// The caret's column within the viewport (`0..width`).
    pub caret_col: u16,
}

/// An editable text buffer with a caret held as a char index (`0..=char_count`).
///
/// All mutating and movement operations act at the caret and keep it on a character boundary.
///
/// ```
/// use tui::app::TextInput;
///
/// let mut input = TextInput::new("café");
/// input.home();          // caret to the start
/// input.move_right();    // past 'c'
/// input.insert_char('x');
/// assert_eq!(input.as_str(), "cxafé");
/// input.delete();        // forward-delete the 'a'
/// assert_eq!(input.as_str(), "cxfé");
/// ```
#[derive(Debug, Clone, PartialEq, Eq, Default)]
pub struct TextInput {
    value: String,
    /// Caret position as a char index into `value`, in `0..=char_count`.
    caret: usize,
}

impl TextInput {
    /// A new input seeded with `value`, caret placed at the end (the natural resume point when a
    /// field is pre-filled from an existing value).
    #[must_use]
    pub fn new(value: impl Into<String>) -> Self {
        let value = value.into();
        let caret = value.chars().count();
        Self { value, caret }
    }

    /// The current buffer contents.
    #[must_use]
    pub fn as_str(&self) -> &str {
        &self.value
    }

    /// The caret position as a char index (`0..=char_count`). Exposed for render + test assertions.
    #[must_use]
    pub fn caret(&self) -> usize {
        self.caret
    }

    /// Whether the buffer is empty.
    #[must_use]
    pub fn is_empty(&self) -> bool {
        self.value.is_empty()
    }

    /// The number of characters in the buffer (not bytes).
    fn char_count(&self) -> usize {
        self.value.chars().count()
    }

    /// The byte offset of char index `idx`, or the buffer length when `idx` is at/after the end.
    /// Keeps every mutation UTF-8-safe without indexing into a multi-byte character.
    fn byte_offset(&self, idx: usize) -> usize {
        self.value
            .char_indices()
            .nth(idx)
            .map_or(self.value.len(), |(b, _)| b)
    }

    /// Insert `c` at the caret and advance the caret past it.
    pub fn insert_char(&mut self, c: char) {
        let at = self.byte_offset(self.caret);
        self.value.insert(at, c);
        self.caret = self.caret.saturating_add(1);
    }

    /// Delete the character before the caret (no-op at the start of the buffer).
    pub fn backspace(&mut self) {
        if self.caret == 0 {
            return;
        }
        let start = self.byte_offset(self.caret - 1);
        let end = self.byte_offset(self.caret);
        let _ = self.value.drain(start..end);
        self.caret -= 1;
    }

    /// Forward-delete the character at the caret (no-op at the end of the buffer). The caret does
    /// not move.
    pub fn delete(&mut self) {
        if self.caret >= self.char_count() {
            return;
        }
        let start = self.byte_offset(self.caret);
        let end = self.byte_offset(self.caret + 1);
        let _ = self.value.drain(start..end);
    }

    /// Move the caret one character left (no-op at the start).
    pub fn move_left(&mut self) {
        self.caret = self.caret.saturating_sub(1);
    }

    /// Move the caret one character right (no-op at the end).
    pub fn move_right(&mut self) {
        if self.caret < self.char_count() {
            self.caret = self.caret.saturating_add(1);
        }
    }

    /// Move the caret to the start of the current line.
    pub fn home(&mut self) {
        let (line, _) = self.caret_line_col();
        if let Some((start, _)) = self.lines_layout().get(line) {
            self.caret = *start;
        }
    }

    /// Move the caret to the end of the current line.
    pub fn end(&mut self) {
        let (line, _) = self.caret_line_col();
        if let Some((start, len)) = self.lines_layout().get(line) {
            self.caret = start.saturating_add(*len);
        }
    }

    /// Move the caret up one logical line, preserving the column where possible (clamped to the
    /// target line's length). At the first line, moves to the start of the buffer.
    pub fn move_up(&mut self) {
        let (line, col) = self.caret_line_col();
        if line == 0 {
            self.caret = 0;
            return;
        }
        let layout = self.lines_layout();
        if let Some((start, len)) = layout.get(line - 1) {
            self.caret = start.saturating_add(col.min(*len));
        }
    }

    /// Move the caret down one logical line, preserving the column where possible (clamped to the
    /// target line's length). At the last line, moves to the end of the buffer.
    pub fn move_down(&mut self) {
        let (line, col) = self.caret_line_col();
        let layout = self.lines_layout();
        if line + 1 >= layout.len() {
            self.caret = self.char_count();
            return;
        }
        if let Some((start, len)) = layout.get(line + 1) {
            self.caret = start.saturating_add(col.min(*len));
        }
    }

    /// The logical lines as `(start_char_index, length_in_chars)` pairs, split on `'\n'`. Always
    /// non-empty — an empty buffer yields one empty line, a trailing `'\n'` a final empty line.
    fn lines_layout(&self) -> Vec<(usize, usize)> {
        let mut lines = Vec::new();
        let mut start = 0usize;
        let mut len = 0usize;
        for c in self.value.chars() {
            if c == '\n' {
                lines.push((start, len));
                start = start.saturating_add(len).saturating_add(1);
                len = 0;
            } else {
                len = len.saturating_add(1);
            }
        }
        lines.push((start, len));
        lines
    }

    /// The caret's `(logical line index, column within the line)`.
    fn caret_line_col(&self) -> (usize, usize) {
        let layout = self.lines_layout();
        let mut last = (0usize, 0usize);
        for (idx, (start, len)) in layout.iter().enumerate() {
            if self.caret <= start.saturating_add(*len) {
                return (idx, self.caret.saturating_sub(*start));
            }
            last = (idx, self.caret.saturating_sub(*start));
        }
        last
    }

    /// The visible substring and caret column for a single-line field `width` columns wide,
    /// horizontally scrolled so a caret past the right edge stays visible. `width == 0` yields an
    /// empty view.
    #[must_use]
    pub fn field_view(&self, width: u16) -> (String, u16) {
        let width = usize::from(width);
        if width == 0 {
            return (String::new(), 0);
        }
        let chars: Vec<char> = self.value.chars().collect();
        let caret = self.caret.min(chars.len());
        let scroll = if caret >= width {
            caret.saturating_sub(width).saturating_add(1)
        } else {
            0
        };
        let visible: String = chars.iter().skip(scroll).take(width).collect();
        let col = u16::try_from(caret.saturating_sub(scroll)).unwrap_or(0);
        (visible, col)
    }

    /// The visible, hard-wrapped rows and caret position for a `width × height` multiline viewport,
    /// vertically scrolled to keep the caret line visible. Wrapping is hard (at `width` columns),
    /// honouring embedded `'\n'`; a caret parked exactly at a wrap boundary sits at the start of the
    /// following (possibly empty) row so the next inserted character's position is shown.
    #[must_use]
    pub fn viewport(&self, width: u16, height: u16) -> Viewport {
        let w = usize::from(width).max(1);
        let h = usize::from(height).max(1);
        let (caret_line, caret_col_line) = self.caret_line_col();
        let chars: Vec<char> = self.value.chars().collect();

        let mut visual: Vec<String> = Vec::new();
        let mut abs_caret_row = 0usize;
        let mut abs_caret_col = 0usize;
        for (li, (start, len)) in self.lines_layout().iter().enumerate() {
            let mut rows: Vec<String> = Vec::new();
            let mut i = 0usize;
            while i < *len {
                let take = w.min(len.saturating_sub(i));
                let row: String = chars
                    .iter()
                    .skip(start.saturating_add(i))
                    .take(take)
                    .collect();
                rows.push(row);
                i = i.saturating_add(w);
            }
            if rows.is_empty() {
                rows.push(String::new());
            }
            if li == caret_line {
                let cvr = caret_col_line / w;
                let cvc = caret_col_line % w;
                while rows.len() <= cvr {
                    rows.push(String::new());
                }
                abs_caret_row = visual.len().saturating_add(cvr);
                abs_caret_col = cvc;
            }
            visual.extend(rows);
        }

        let scroll = abs_caret_row.saturating_sub(h.saturating_sub(1));
        let lines: Vec<String> = visual.into_iter().skip(scroll).take(h).collect();
        Viewport {
            lines,
            caret_row: u16::try_from(abs_caret_row.saturating_sub(scroll)).unwrap_or(0),
            caret_col: u16::try_from(abs_caret_col).unwrap_or(0),
        }
    }
}

impl From<String> for TextInput {
    fn from(value: String) -> Self {
        Self::new(value)
    }
}

/// Apply a caret-movement / forward-delete [`Event`](super::Event) to `input`, returning `true`
/// when the event was a text-motion event (and was applied). `Char` and `Backspace` are **not**
/// handled here — a field handler owns those so it can apply field-specific semantics (e.g. the
/// numeric fields' digit filtering). This lets every field route the movement keys through one arm.
#[must_use]
pub fn apply_motion(input: &mut TextInput, event: &super::Event) -> bool {
    use super::Event;
    match event {
        Event::MoveLeft => input.move_left(),
        Event::MoveRight => input.move_right(),
        Event::MoveHome => input.home(),
        Event::MoveEnd => input.end(),
        Event::MoveUp => input.move_up(),
        Event::MoveDown => input.move_down(),
        Event::Delete => input.delete(),
        _ => return false,
    }
    true
}
