//! Candidate list management
//!
//! Handles the list of conversion candidates with pagination support.

/// A single conversion candidate.
///
/// Two distinct annotation slots are kept separate so the same description
/// never appears in two places at once:
///
/// - `source_label` — shown in the aux text (after the model name) to tell
///   the user which subsystem produced the candidate (`🤖 AI`, `📚 辞書`,
///   `📝 学習`, `🔄 変換`, ...).
/// - `description` — shown as the mozc-style right-side comment on the
///   candidate itself, describing what the candidate *is* (symbol names like
///   `三点リーダ`, rewriter variants like `[全]英大文字`).
///
/// Position within a `CandidateList` is tracked by the list itself; the
/// candidate doesn't carry its own index.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Candidate {
    /// The converted text
    pub text: String,
    /// The original reading (hiragana)
    pub reading: Option<String>,
    /// Source label for the aux text slot (e.g. `🤖 AI`, `📚 辞書`).
    /// `None` when the source has no label (Fallback).
    pub source_label: Option<String>,
    /// Per-candidate description shown as the right-side comment on the
    /// candidate (mozc-style). Only set when the candidate itself has a
    /// meaningful description — symbol descriptions like `三点リーダ`,
    /// rewriter descriptions like `[全]英大文字`. Source labels are
    /// intentionally excluded so they don't duplicate the aux text.
    pub description: Option<String>,
}

impl Candidate {
    pub fn new(text: impl Into<String>) -> Self {
        Self {
            text: text.into(),
            reading: None,
            source_label: None,
            description: None,
        }
    }

    pub fn with_reading(text: impl Into<String>, reading: impl Into<String>) -> Self {
        Self {
            text: text.into(),
            reading: Some(reading.into()),
            source_label: None,
            description: None,
        }
    }
}

impl From<String> for Candidate {
    fn from(text: String) -> Self {
        Self::new(text)
    }
}

impl From<&str> for Candidate {
    fn from(text: &str) -> Self {
        Self::new(text)
    }
}

/// A list of candidates with pagination and selection support
#[derive(Debug, Clone)]
pub struct CandidateList {
    /// All candidates
    candidates: Vec<Candidate>,
    /// Currently selected candidate index. `None` means the list is visible
    /// as a prediction window, but no row is highlighted yet.
    cursor: Option<usize>,
    /// Number of candidates per page
    page_size: usize,
}

impl CandidateList {
    /// Default page size for candidate display
    pub const DEFAULT_PAGE_SIZE: usize = 9;

    /// Create a new candidate list
    pub fn new(candidates: Vec<Candidate>) -> Self {
        Self {
            candidates,
            cursor: Some(0),
            page_size: Self::DEFAULT_PAGE_SIZE,
        }
    }

    /// Create a candidate list with no selected row.
    pub fn new_unselected(candidates: Vec<Candidate>) -> Self {
        Self {
            candidates,
            cursor: None,
            page_size: Self::DEFAULT_PAGE_SIZE,
        }
    }

    /// Create a candidate list from strings
    pub fn from_strings(strings: impl IntoIterator<Item = impl Into<String>>) -> Self {
        Self::new(strings.into_iter().map(Candidate::new).collect())
    }

    /// Create a candidate list from strings, attaching the same reading to
    /// every candidate.
    pub fn from_strings_with_reading(
        strings: impl IntoIterator<Item = impl Into<String>>,
        reading: impl Into<String>,
    ) -> Self {
        let reading = reading.into();
        Self::new(
            strings
                .into_iter()
                .map(|s| Candidate::with_reading(s, &reading))
                .collect(),
        )
    }

    /// Get all candidates
    pub fn candidates(&self) -> &[Candidate] {
        &self.candidates
    }

    /// Get the number of candidates
    pub fn len(&self) -> usize {
        self.candidates.len()
    }

    /// Check if empty
    pub fn is_empty(&self) -> bool {
        self.candidates.is_empty()
    }

    /// Get the current cursor position
    pub fn cursor(&self) -> Option<usize> {
        self.cursor
    }

    /// Get the page size
    pub fn page_size(&self) -> usize {
        self.page_size
    }

    /// Get the current page number (0-indexed)
    pub fn current_page(&self) -> usize {
        self.cursor
            .and_then(|cursor| cursor.checked_div(self.page_size))
            .unwrap_or(0)
    }

    /// Get the total number of pages
    pub fn total_pages(&self) -> usize {
        if self.page_size == 0 || self.candidates.is_empty() {
            0
        } else {
            self.candidates.len().div_ceil(self.page_size)
        }
    }

    /// Get the start index of the current page
    pub fn page_start(&self) -> usize {
        self.current_page() * self.page_size
    }

    /// Get the candidates for the current page
    pub fn page_candidates(&self) -> &[Candidate] {
        let start = self.page_start();
        let end = (start + self.page_size).min(self.candidates.len());
        &self.candidates[start..end]
    }

    /// Get the cursor position within the current page (0-indexed)
    pub fn page_cursor(&self) -> Option<usize> {
        self.cursor.map(|cursor| cursor - self.page_start())
    }

    /// Get the currently selected candidate
    pub fn selected(&self) -> Option<&Candidate> {
        self.cursor.and_then(|cursor| self.candidates.get(cursor))
    }

    /// Get the currently selected text
    pub fn selected_text(&self) -> Option<&str> {
        self.selected().map(|c| c.text.as_str())
    }

    /// Move to the next candidate
    pub fn move_next(&mut self) -> bool {
        let Some(cursor) = self.cursor else {
            if self.candidates.is_empty() {
                return false;
            }
            self.cursor = Some(0);
            return true;
        };
        if cursor + 1 < self.candidates.len() {
            self.cursor = Some(cursor + 1);
            true
        } else if !self.candidates.is_empty() {
            // Wrap to beginning
            self.cursor = Some(0);
            true
        } else {
            false
        }
    }

    /// Move to the previous candidate
    pub fn move_prev(&mut self) -> bool {
        let Some(cursor) = self.cursor else {
            if self.candidates.is_empty() {
                return false;
            }
            self.cursor = Some(self.candidates.len() - 1);
            return true;
        };
        if cursor > 0 {
            self.cursor = Some(cursor - 1);
            true
        } else if !self.candidates.is_empty() {
            // Wrap to end
            self.cursor = Some(self.candidates.len() - 1);
            true
        } else {
            false
        }
    }

    /// Move to the next page
    pub fn next_page(&mut self) -> bool {
        if self.candidates.is_empty() {
            return false;
        }

        let next_page_start = self.page_start() + self.page_size;
        if next_page_start < self.candidates.len() {
            self.cursor = Some(next_page_start);
            true
        } else {
            // Wrap to first page
            self.cursor = Some(0);
            true
        }
    }

    /// Move to the previous page
    pub fn prev_page(&mut self) -> bool {
        if self.candidates.is_empty() {
            return false;
        }

        let current_page = self.current_page();
        if current_page > 0 {
            self.cursor = Some((current_page - 1) * self.page_size);
            true
        } else {
            // Wrap to last page
            let last_page = self.total_pages().saturating_sub(1);
            self.cursor = Some(last_page * self.page_size);
            true
        }
    }

    /// Select a candidate by index within the current page (1-9)
    pub fn select_on_page(&mut self, page_index: usize) -> Option<&Candidate> {
        if page_index == 0 || page_index > self.page_size {
            return None;
        }

        let absolute_index = self.page_start() + page_index - 1;
        if absolute_index < self.candidates.len() {
            self.cursor = Some(absolute_index);
            self.selected()
        } else {
            None
        }
    }

    /// Select a candidate by absolute index
    pub fn select(&mut self, index: usize) -> Option<&Candidate> {
        if index < self.candidates.len() {
            self.cursor = Some(index);
            self.selected()
        } else {
            None
        }
    }

    /// Reset cursor to beginning
    pub fn reset(&mut self) {
        self.cursor = Some(0);
    }

    /// Clear selection while keeping the candidates visible.
    pub fn clear_selection(&mut self) {
        self.cursor = None;
    }

    /// Update the candidate list with new candidates
    pub fn update(&mut self, candidates: Vec<Candidate>) {
        self.candidates = candidates;
        self.cursor = Some(0);
    }
}

impl Default for CandidateList {
    fn default() -> Self {
        Self::new(Vec::new())
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_candidate_list_basic() {
        let candidates = CandidateList::from_strings(["今日", "京", "恭"]);
        assert_eq!(candidates.len(), 3);
        assert_eq!(candidates.selected_text(), Some("今日"));
    }

    #[test]
    fn test_candidate_list_unselected() {
        let mut candidates = CandidateList::new_unselected(vec![Candidate::new("今日")]);
        assert_eq!(candidates.cursor(), None);
        assert_eq!(candidates.page_cursor(), None);
        assert_eq!(candidates.selected_text(), None);

        assert!(candidates.move_next());
        assert_eq!(candidates.cursor(), Some(0));
        assert_eq!(candidates.selected_text(), Some("今日"));
    }

    #[test]
    fn test_candidate_list_navigation() {
        let mut candidates = CandidateList::from_strings(["a", "b", "c"]);

        assert!(candidates.move_next());
        assert_eq!(candidates.selected_text(), Some("b"));

        assert!(candidates.move_next());
        assert_eq!(candidates.selected_text(), Some("c"));

        // Wrap around
        assert!(candidates.move_next());
        assert_eq!(candidates.selected_text(), Some("a"));

        // Wrap back
        assert!(candidates.move_prev());
        assert_eq!(candidates.selected_text(), Some("c"));
    }

    #[test]
    fn test_candidate_list_pagination() {
        // Default page_size is 9, so 20 items = 3 pages (9+9+2)
        let items: Vec<_> = (1..=20).map(|i| format!("item{}", i)).collect();
        let mut candidates = CandidateList::from_strings(items);

        assert_eq!(candidates.total_pages(), 3);
        assert_eq!(candidates.current_page(), 0);
        assert_eq!(candidates.page_candidates().len(), 9);

        candidates.next_page();
        assert_eq!(candidates.current_page(), 1);
        assert_eq!(candidates.page_start(), 9);

        candidates.next_page();
        assert_eq!(candidates.current_page(), 2);
        assert_eq!(candidates.page_candidates().len(), 2);

        // Wrap to first page
        candidates.next_page();
        assert_eq!(candidates.current_page(), 0);
    }

    #[test]
    fn test_candidate_list_select_on_page() {
        let items: Vec<_> = (1..=20).map(|i| format!("item{}", i)).collect();
        let mut candidates = CandidateList::from_strings(items);

        // Select item 3 on first page
        candidates.select_on_page(3);
        assert_eq!(candidates.selected_text(), Some("item3"));

        // Move to second page and select item 2
        candidates.next_page();
        candidates.select_on_page(2);
        assert_eq!(candidates.selected_text(), Some("item11")); // 9 + 2 = 11
    }
}
