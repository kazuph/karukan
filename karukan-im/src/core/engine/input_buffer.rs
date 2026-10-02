//! InputBuffer: composed hiragana text with cursor.
//!
//! This struct bundles `text` and `cursor_pos`
//! which are always operated on together.

/// Composed input buffer with cursor.
pub(super) struct InputBuffer {
    /// Composed hiragana text (source of truth)
    pub text: String,
    /// Cursor position (in characters, not bytes)
    pub cursor_pos: usize,
    /// Raw keystrokes that produced each char of `text`, parallel by char
    /// index. A romaji conversion group keeps its whole key sequence on the
    /// first char it emitted (e.g. `き` ← "ki", `ょ` ← ""), so a prefix of
    /// the buffer yields the prefix's keys by simple concatenation. Used by
    /// the alphabet transliterations (F9/F10, Ctrl+L/:/'), which reproduce
    /// the keys the user actually typed instead of re-romanizing the kana.
    pub keys: Vec<String>,
    /// Keystrokes sitting in the romaji converter's buffer — typed but not
    /// yet emitted as text (e.g. a pending "n"). Moved into `keys` of the
    /// first char emitted by the next conversion or flush.
    pub pending_keys: String,
}

impl InputBuffer {
    /// Create a new empty buffer.
    pub fn new() -> Self {
        Self {
            text: String::new(),
            cursor_pos: 0,
            keys: Vec::new(),
            pending_keys: String::new(),
        }
    }

    /// Clear the buffer (text, cursor, key tracking).
    pub fn clear(&mut self) {
        self.text.clear();
        self.cursor_pos = 0;
        self.keys.clear();
        self.pending_keys.clear();
    }

    /// Insert text at the current cursor position.
    ///
    /// `keys` is the raw key sequence that produced the whole inserted text;
    /// it is attached to the first inserted char (see `keys` field doc).
    pub fn insert(&mut self, text: &str, keys: &str) {
        if text.is_empty() {
            return;
        }
        let byte_pos = self
            .text
            .char_indices()
            .nth(self.cursor_pos)
            .map(|(i, _)| i)
            .unwrap_or(self.text.len());
        self.text.insert_str(byte_pos, text);
        let char_count = text.chars().count();
        // `keys` can fall behind `text` when tests assign `text` directly;
        // pad so the splice position is always valid.
        if self.keys.len() < self.cursor_pos {
            self.keys.resize(self.cursor_pos, String::new());
        }
        let mut entries = Vec::with_capacity(char_count);
        entries.push(keys.to_string());
        entries.resize(char_count, String::new());
        self.keys.splice(self.cursor_pos..self.cursor_pos, entries);
        self.cursor_pos += char_count;
    }

    /// Remove the character at the given character position.
    pub fn remove_char_at(&mut self, char_pos: usize) -> Option<char> {
        let (byte_start, removed) = self.text.char_indices().nth(char_pos)?;
        let byte_end = self
            .text
            .char_indices()
            .nth(char_pos + 1)
            .map(|(i, _)| i)
            .unwrap_or(self.text.len());
        self.text.replace_range(byte_start..byte_end, "");
        if char_pos < self.keys.len() {
            self.keys.remove(char_pos);
        }
        Some(removed)
    }

    /// Remove the character before the cursor.
    pub fn remove_char_before_cursor(&mut self) -> Option<char> {
        if self.cursor_pos == 0 {
            return None;
        }
        self.cursor_pos -= 1;
        self.remove_char_at(self.cursor_pos)
    }

    /// Remove the character at the cursor position (delete key).
    pub fn remove_char_at_cursor(&mut self) -> Option<char> {
        self.remove_char_at(self.cursor_pos)
    }

    /// Split off the keystrokes that were just emitted as text: all pending
    /// keys except the last `remaining`, which still sit in the romaji
    /// converter's buffer. Callers pass `romaji.buffer().chars().count()`
    /// after a push, or 0 after a flush.
    pub fn take_emitted_keys(&mut self, remaining: usize) -> String {
        let total = self.pending_keys.chars().count();
        let emitted = total.saturating_sub(remaining);
        let emitted_keys: String = self.pending_keys.chars().take(emitted).collect();
        self.pending_keys = self.pending_keys.chars().skip(emitted).collect();
        emitted_keys
    }

    /// Concatenated raw keystrokes that produced `text[..end]` (char count).
    ///
    /// Chars with recorded keys contribute those keys. Chars without any —
    /// e.g. text assigned directly in tests, or group-remainder chars — fall
    /// back to themselves when ASCII; non-ASCII chars without keys
    /// contribute nothing (their keys already live on the group's first
    /// char).
    pub fn raw_keys(&self, end: usize) -> String {
        let mut out = String::new();
        for (i, c) in self.text.chars().take(end).enumerate() {
            match self.keys.get(i).map(String::as_str) {
                Some(k) if !k.is_empty() => out.push_str(k),
                _ => {
                    if c.is_ascii() {
                        out.push(c);
                    }
                }
            }
        }
        out
    }
}
