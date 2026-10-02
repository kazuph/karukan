//! Transliteration ("表示し直し") keys — Google日本語入力 compatible
//! (same assignments as Mozc's `src/data/keymap/kotoeri.tsv` defaults):
//!
//! | key                    | form       |
//! |------------------------|------------|
//! | Ctrl+J / F6            | ひらがな   |
//! | Ctrl+K / F7            | 全角カタカナ |
//! | Ctrl+; / F8            | 半角       |
//! | Ctrl+L / F9            | 全角英数   |
//! | Ctrl+: / Ctrl+' / F10  | 半角英数   |
//!
//! Pressing one while composing re-displays the whole reading in that
//! form and enters the Conversion state (Enter commits it, further
//! typing commits and starts a new input — the usual conversion
//! behaviour). Pressing one during conversion applies the form to the
//! focused segment only. The alphabet forms reproduce the keys the user
//! actually typed (InputBuffer::raw_keys); pressing an alphabet key
//! again cycles 小文字 → 大文字 → 先頭だけ大文字 → 小文字…

use karukan_engine::kana::ascii_to_fullwidth_char;

use super::*;

/// The form a transliteration key asks for.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(in crate::core) enum Transliteration {
    /// ひらがな (Ctrl+J, F6)
    Hiragana,
    /// 全角カタカナ (Ctrl+K, F7)
    Katakana,
    /// 半角: kana → half-width katakana, full-width alphanumeric →
    /// half-width alphanumeric (Ctrl+;, F8)
    HalfWidth,
    /// 全角英数 (Ctrl+L, F9)
    FullAlphabet,
    /// 半角英数 (Ctrl+:, Ctrl+', F10)
    HalfAlphabet,
}

impl Transliteration {
    /// Map a key press to a transliteration kind, or `None` if it isn't one
    /// of the transliteration shortcuts. Key releases never reach here —
    /// `process_key` drops them earlier.
    pub(super) fn from_key(key: &KeyEvent) -> Option<Self> {
        if key.modifiers.control_key && !key.modifiers.alt_key {
            return Some(match key.keysym {
                Keysym::KEY_J | Keysym::KEY_J_UPPER => Self::Hiragana,
                Keysym::KEY_K | Keysym::KEY_K_UPPER => Self::Katakana,
                Keysym::SEMICOLON => Self::HalfWidth,
                Keysym::KEY_L | Keysym::KEY_L_UPPER => Self::FullAlphabet,
                // JIS has a dedicated `:` key, US puts `'` on the same
                // physical key; both mean 半角英数.
                Keysym::COLON | Keysym::APOSTROPHE => Self::HalfAlphabet,
                _ => return None,
            });
        }
        if key.modifiers.control_key || key.modifiers.alt_key {
            return None;
        }
        match key.keysym {
            Keysym::F6 => Some(Self::Hiragana),
            Keysym::F7 => Some(Self::Katakana),
            Keysym::F8 => Some(Self::HalfWidth),
            Keysym::F9 => Some(Self::FullAlphabet),
            Keysym::F10 => Some(Self::HalfAlphabet),
            _ => None,
        }
    }
}

/// One alphabet form of `keys` at case `step`: 0 = 小文字, 1 = 大文字,
/// 2 = 先頭だけ大文字. `full_width` selects 全角/半角.
fn alphabet_form(keys: &str, full_width: bool, step: usize) -> String {
    let lower = keys.to_ascii_lowercase();
    let half = match step % 3 {
        1 => lower.to_ascii_uppercase(),
        2 => {
            let mut chars = lower.chars();
            match chars.next() {
                Some(c) => c.to_uppercase().collect::<String>() + chars.as_str(),
                None => String::new(),
            }
        }
        _ => lower,
    };
    if full_width {
        half.chars().map(ascii_to_fullwidth_char).collect()
    } else {
        half
    }
}

/// All alphabet case variants for `keys` at one width, in cycle order.
fn alphabet_variants(keys: &str, full_width: bool) -> [String; 3] {
    [
        alphabet_form(keys, full_width, 0),
        alphabet_form(keys, full_width, 1),
        alphabet_form(keys, full_width, 2),
    ]
}

/// Which alphabet form to show. Repeated presses of an alphabet key cycle
/// 小文字→大文字→先頭だけ大文字: when the currently selected surface already
/// is one of this width's forms, the next one is shown; otherwise the cycle
/// starts at 小文字.
fn next_alphabet_form(keys: &str, full_width: bool, current: Option<&str>) -> String {
    let variants = alphabet_variants(keys, full_width);
    let step = current
        .and_then(|s| variants.iter().position(|v| v == s))
        .map(|i| i + 1)
        .unwrap_or(0)
        % 3;
    variants[step].clone()
}

/// The non-alphabet forms a transliteration key can produce.
fn kana_form(kind: Transliteration, reading: &str) -> String {
    match kind {
        Transliteration::Hiragana => karukan_engine::katakana_to_hiragana(reading),
        Transliteration::Katakana => karukan_engine::hiragana_to_katakana(reading),
        Transliteration::HalfWidth => karukan_engine::to_half_width(reading),
        Transliteration::FullAlphabet | Transliteration::HalfAlphabet => unreachable!(),
    }
}

fn form_description(kind: Transliteration) -> &'static str {
    match kind {
        Transliteration::Hiragana => "ひらがな",
        Transliteration::Katakana => "全角カタカナ",
        Transliteration::HalfWidth => "半角",
        Transliteration::FullAlphabet => "全角英数",
        Transliteration::HalfAlphabet => "半角英数",
    }
}

impl InputMethodEngine {
    /// Handle a transliteration key press in Composing or Conversion state.
    pub(super) fn transliterate(&mut self, kind: Transliteration) -> EngineResult {
        match &self.state {
            InputState::Composing { .. } => self.transliterate_composing(kind),
            InputState::Conversion { .. } => self.transliterate_conversion(kind),
            InputState::Empty => EngineResult::not_consumed(),
        }
    }

    /// Re-display the whole reading in the requested form and enter the
    /// Conversion state with the transliteration variants as candidates.
    fn transliterate_composing(&mut self, kind: Transliteration) -> EngineResult {
        self.flush_romaji_to_composed();
        let reading = self.input_buf.text.clone();
        if reading.is_empty() {
            return EngineResult::consumed();
        }
        let len = reading.chars().count();
        let keys = self.input_buf.raw_keys(len);
        self.converters.romaji.reset();
        self.conversion_history.clear();
        self.input_buf.cursor_pos = len;
        self.live.text.clear();
        self.chunks.clear();

        let (variants, selected) = Self::transliteration_variants(&reading, &keys, kind, None);
        let mut candidates = CandidateList::new(variants);
        candidates.select(selected);
        self.enter_conversion_state(&reading, candidates)
    }

    /// Apply the requested form to the focused conversion segment: select
    /// the matching candidate, appending it to the candidate list when it
    /// isn't already there (alphabet forms are absent unless the reading
    /// was pure alphabet to begin with).
    fn transliterate_conversion(&mut self, kind: Transliteration) -> EngineResult {
        let target_len = self.conversion_target_len();
        let reading = self.conversion_target_reading();
        if reading.is_empty() {
            return EngineResult::consumed();
        }
        let keys = self.input_buf.raw_keys(target_len);
        let current = self
            .state
            .candidates()
            .and_then(|c| c.selected_text().map(str::to_string));
        let Some(desired) = Self::transliterated_surface(kind, &reading, &keys, current.as_deref())
        else {
            return EngineResult::consumed();
        };

        let candidates = {
            let Some(list) = self.state.candidates_mut() else {
                return EngineResult::not_consumed();
            };
            match list.candidates().iter().position(|c| c.text == desired) {
                Some(index) => {
                    list.select(index);
                }
                None => {
                    let mut all = list.candidates().to_vec();
                    all.push(Self::transliteration_candidate(
                        desired.clone(),
                        &reading,
                        form_description(kind),
                    ));
                    let index = all.len() - 1;
                    list.update(all);
                    list.select(index);
                }
            }
            list.clone()
        };
        let selected_text = candidates.selected_text().unwrap_or("").to_string();
        self.update_conversion_preedit(&selected_text, &candidates)
    }

    /// The surface one transliteration key produces for `reading`/`keys`.
    /// `current` is the surface currently selected (for alphabet cycling).
    /// Returns `None` when the form cannot be produced (alphabet forms need
    /// recorded keystrokes).
    fn transliterated_surface(
        kind: Transliteration,
        reading: &str,
        keys: &str,
        current: Option<&str>,
    ) -> Option<String> {
        match kind {
            Transliteration::Hiragana | Transliteration::Katakana | Transliteration::HalfWidth => {
                Some(kana_form(kind, reading))
            }
            Transliteration::FullAlphabet | Transliteration::HalfAlphabet => {
                if keys.is_empty() {
                    None
                } else {
                    Some(next_alphabet_form(
                        keys,
                        kind == Transliteration::FullAlphabet,
                        current,
                    ))
                }
            }
        }
    }

    fn transliteration_candidate(
        text: String,
        reading: &str,
        description: &'static str,
    ) -> Candidate {
        Candidate {
            text,
            reading: Some(reading.to_string()),
            source_label: Some(CandidateSource::Rewriter.label().to_string()),
            description: Some(description.to_string()),
        }
    }

    /// The transliteration variants of `reading` as a candidate list plus
    /// the index of the variant `kind` asks for. Ordering mirrors Mozc:
    /// ひらがな, 全角カタカナ, 半角, then the 全角/半角英数 case variants.
    /// Duplicates and empty variants are dropped; the desired variant's
    /// first occurrence is selected.
    fn transliteration_variants(
        reading: &str,
        keys: &str,
        kind: Transliteration,
        current: Option<&str>,
    ) -> (Vec<Candidate>, usize) {
        let desired = Self::transliterated_surface(kind, reading, keys, current);

        let mut variants: Vec<Candidate> = Vec::new();
        let mut push_unique = |text: String, kind: Transliteration| {
            if !text.is_empty() && !variants.iter().any(|c| c.text == text) {
                variants.push(Self::transliteration_candidate(
                    text,
                    reading,
                    form_description(kind),
                ));
            }
        };
        push_unique(
            kana_form(Transliteration::Hiragana, reading),
            Transliteration::Hiragana,
        );
        push_unique(
            kana_form(Transliteration::Katakana, reading),
            Transliteration::Katakana,
        );
        push_unique(
            kana_form(Transliteration::HalfWidth, reading),
            Transliteration::HalfWidth,
        );
        for text in alphabet_variants(keys, true) {
            push_unique(text, Transliteration::FullAlphabet);
        }
        for text in alphabet_variants(keys, false) {
            push_unique(text, Transliteration::HalfAlphabet);
        }

        let selected = desired
            .and_then(|d| variants.iter().position(|c| c.text == d))
            .unwrap_or(0);
        (variants, selected)
    }
}
