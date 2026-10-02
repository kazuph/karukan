use super::*;

// --- Transliteration ("表示し直し") key tests ---
//
// Google日本語入力 compatible assignments (Mozc kotoeri.tsv defaults):
//   Ctrl+J / F6           → ひらがな
//   Ctrl+K / F7           → 全角カタカナ
//   Ctrl+; / F8           → 半角
//   Ctrl+L / F9           → 全角英数
//   Ctrl+: / Ctrl+' / F10 → 半角英数
//
// Composing: the key re-displays the whole reading and enters Conversion.
// Conversion: the key applies the form to the focused segment.
// Alphabet keys cycle 小文字 → 大文字 → 先頭だけ大文字 → 小文字…

fn type_str(engine: &mut InputMethodEngine, s: &str) {
    for ch in s.chars() {
        engine.process_key(&press(ch));
    }
}

/// Extract the committed text from an EngineResult's actions.
fn committed_text(result: &EngineResult) -> Option<String> {
    result.actions.iter().find_map(|a| match a {
        EngineAction::Commit(text) => Some(text.clone()),
        _ => None,
    })
}

fn assert_preedit(engine: &InputMethodEngine, expected: &str) {
    assert_eq!(
        engine.preedit().map(|p| p.text().to_string()),
        Some(expected.to_string())
    );
    assert!(
        matches!(engine.state(), InputState::Conversion { .. }),
        "transliteration should leave the engine in Conversion state"
    );
}

// --- Each key in Composing state ---

#[test]
fn test_ctrl_j_shows_hiragana() {
    let mut engine = InputMethodEngine::new();
    type_str(&mut engine, "nihongo");
    assert_eq!(engine.preedit().unwrap().text(), "にほんご");

    let result = engine.process_key(&press_ctrl(Keysym::KEY_J));
    assert!(result.consumed);
    assert_preedit(&engine, "にほんご");
}

#[test]
fn test_f6_shows_hiragana() {
    let mut engine = InputMethodEngine::new();
    type_str(&mut engine, "nihongo");

    let result = engine.process_key(&press_key(Keysym::F6));
    assert!(result.consumed);
    assert_preedit(&engine, "にほんご");
}

#[test]
fn test_ctrl_k_shows_full_katakana() {
    let mut engine = InputMethodEngine::new();
    type_str(&mut engine, "nihongo");

    let result = engine.process_key(&press_ctrl(Keysym::KEY_K));
    assert!(result.consumed);
    assert_preedit(&engine, "ニホンゴ");
    // Ctrl+K is a transliteration now, not a mode switch.
    assert_eq!(engine.input_mode, InputMode::Hiragana);
}

#[test]
fn test_f7_shows_full_katakana() {
    let mut engine = InputMethodEngine::new();
    type_str(&mut engine, "nihongo");

    let result = engine.process_key(&press_key(Keysym::F7));
    assert!(result.consumed);
    assert_preedit(&engine, "ニホンゴ");
}

#[test]
fn test_ctrl_semicolon_shows_half_width_katakana() {
    let mut engine = InputMethodEngine::new();
    type_str(&mut engine, "nihongo");

    let result = engine.process_key(&press_ctrl(Keysym::SEMICOLON));
    assert!(result.consumed);
    assert_preedit(&engine, "ﾆﾎﾝｺﾞ");
}

#[test]
fn test_f8_shows_half_width_katakana() {
    let mut engine = InputMethodEngine::new();
    type_str(&mut engine, "gakkou");

    let result = engine.process_key(&press_key(Keysym::F8));
    assert!(result.consumed);
    assert_preedit(&engine, "ｶﾞｯｺｳ");
}

#[test]
fn test_ctrl_l_shows_full_width_alphabet() {
    let mut engine = InputMethodEngine::new();
    type_str(&mut engine, "nihongo");

    let result = engine.process_key(&press_ctrl(Keysym::KEY_L));
    assert!(result.consumed);
    assert_preedit(&engine, "ｎｉｈｏｎｇｏ");
}

#[test]
fn test_f9_shows_full_width_alphabet() {
    let mut engine = InputMethodEngine::new();
    type_str(&mut engine, "nihongo");

    let result = engine.process_key(&press_key(Keysym::F9));
    assert!(result.consumed);
    assert_preedit(&engine, "ｎｉｈｏｎｇｏ");
}

#[test]
fn test_ctrl_colon_shows_half_width_alphabet() {
    let mut engine = InputMethodEngine::new();
    type_str(&mut engine, "nihongo");

    let result = engine.process_key(&press_ctrl(Keysym::COLON));
    assert!(result.consumed);
    assert_preedit(&engine, "nihongo");
}

#[test]
fn test_ctrl_apostrophe_shows_half_width_alphabet() {
    let mut engine = InputMethodEngine::new();
    type_str(&mut engine, "nihongo");

    let result = engine.process_key(&press_ctrl(Keysym::APOSTROPHE));
    assert!(result.consumed);
    assert_preedit(&engine, "nihongo");
}

#[test]
fn test_f10_shows_half_width_alphabet() {
    let mut engine = InputMethodEngine::new();
    type_str(&mut engine, "nihongo");

    let result = engine.process_key(&press_key(Keysym::F10));
    assert!(result.consumed);
    assert_preedit(&engine, "nihongo");
}

// --- Behaviour after transliteration (Conversion-state rules) ---

#[test]
fn test_enter_commits_transliterated_text() {
    let mut engine = InputMethodEngine::new();
    type_str(&mut engine, "nihongo");
    engine.process_key(&press_key(Keysym::F7));

    let result = engine.process_key(&press_key(Keysym::RETURN));
    assert_eq!(committed_text(&result).as_deref(), Some("ニホンゴ"));
    assert!(matches!(engine.state(), InputState::Empty));
}

#[test]
fn test_typing_after_transliteration_commits_and_starts_new_input() {
    let mut engine = InputMethodEngine::new();
    type_str(&mut engine, "nihongo");
    engine.process_key(&press_key(Keysym::F7));

    // Typing 'a' commits the katakana form and starts a new input
    let result = engine.process_key(&press('a'));
    assert_eq!(committed_text(&result).as_deref(), Some("ニホンゴ"));
    assert!(matches!(engine.state(), InputState::Composing { .. }));
    assert_eq!(engine.preedit().unwrap().text(), "あ");
}

#[test]
fn test_escape_returns_to_composing_with_reading() {
    let mut engine = InputMethodEngine::new();
    type_str(&mut engine, "nihongo");
    engine.process_key(&press_key(Keysym::F7));
    assert_preedit(&engine, "ニホンゴ");

    let result = engine.process_key(&press_key(Keysym::ESCAPE));
    assert!(result.consumed);
    assert!(matches!(engine.state(), InputState::Composing { .. }));
    assert_eq!(engine.preedit().unwrap().text(), "にほんご");
}

// --- Transliteration keys in Conversion state act on the focused segment ---

#[test]
fn test_transliteration_keys_inside_conversion() {
    let mut engine = InputMethodEngine::new();
    type_str(&mut engine, "nihongo");
    // Enter conversion via another transliteration key first.
    engine.process_key(&press_key(Keysym::F6));
    assert_preedit(&engine, "にほんご");

    engine.process_key(&press_key(Keysym::F7));
    assert_preedit(&engine, "ニホンゴ");

    engine.process_key(&press_ctrl(Keysym::SEMICOLON));
    assert_preedit(&engine, "ﾆﾎﾝｺﾞ");

    engine.process_key(&press_key(Keysym::F9));
    assert_preedit(&engine, "ｎｉｈｏｎｇｏ");

    engine.process_key(&press_ctrl(Keysym::COLON));
    assert_preedit(&engine, "nihongo");

    engine.process_key(&press_key(Keysym::F6));
    assert_preedit(&engine, "にほんご");
}

#[test]
fn test_conversion_segment_transliteration() {
    let mut engine = InputMethodEngine::new();
    type_str(&mut engine, "watasinonamae");

    // Enter conversion through transliteration, then accept the first
    // segment ("わたし" is the natural bunsetsu split via Shift+Left).
    engine.process_key(&press_key(Keysym::F6));
    assert!(matches!(engine.state(), InputState::Conversion { .. }));

    // Shrink the conversion target to just "わたし".
    for _ in 0.."のなまえ".chars().count() {
        engine.process_key(&press_shift_key(Keysym::LEFT));
    }
    assert_eq!(engine.input_buf.cursor_pos, "わたし".chars().count());

    // F7 applies to the selected segment only; the remainder stays as typed.
    engine.process_key(&press_key(Keysym::F7));
    assert_eq!(engine.preedit().unwrap().text(), "ワタシのなまえ");

    // Advancing commits the transliterated segment, and the remaining
    // segment can be transliterated too (its selected surface depends on
    // whether a conversion model is loaded, so only assert the end state).
    engine.process_key(&press_key(Keysym::RIGHT));
    engine.process_key(&press_key(Keysym::F10));
    assert_eq!(engine.preedit().unwrap().text(), "ワタシnonamae");
}

// --- Alphabet case cycling ---

#[test]
fn test_half_width_alphabet_cycles_case() {
    let mut engine = InputMethodEngine::new();
    type_str(&mut engine, "nihongo");

    engine.process_key(&press_key(Keysym::F10));
    assert_preedit(&engine, "nihongo");

    engine.process_key(&press_key(Keysym::F10));
    assert_preedit(&engine, "NIHONGO");

    engine.process_key(&press_key(Keysym::F10));
    assert_preedit(&engine, "Nihongo");

    engine.process_key(&press_key(Keysym::F10));
    assert_preedit(&engine, "nihongo");
}

#[test]
fn test_full_width_alphabet_cycles_case() {
    let mut engine = InputMethodEngine::new();
    type_str(&mut engine, "nihongo");

    engine.process_key(&press_key(Keysym::F9));
    assert_preedit(&engine, "ｎｉｈｏｎｇｏ");

    engine.process_key(&press_ctrl(Keysym::KEY_L));
    assert_preedit(&engine, "ＮＩＨＯＮＧＯ");

    engine.process_key(&press_key(Keysym::F9));
    assert_preedit(&engine, "Ｎｉｈｏｎｇｏ");

    engine.process_key(&press_key(Keysym::F9));
    assert_preedit(&engine, "ｎｉｈｏｎｇｏ");
}

#[test]
fn test_alphabet_cycle_restarts_at_lower_after_other_transliteration() {
    let mut engine = InputMethodEngine::new();
    type_str(&mut engine, "nihongo");

    engine.process_key(&press_key(Keysym::F10));
    engine.process_key(&press_key(Keysym::F10));
    assert_preedit(&engine, "NIHONGO");

    // Switching to another form, then back, restarts the case cycle.
    engine.process_key(&press_key(Keysym::F7));
    assert_preedit(&engine, "ニホンゴ");
    engine.process_key(&press_key(Keysym::F10));
    assert_preedit(&engine, "nihongo");
}

#[test]
fn test_alphabet_uses_typed_keys_for_mixed_input() {
    let mut engine = InputMethodEngine::new();
    // "kyouha" + Shift+A + "bc" → きょうはAbc
    type_str(&mut engine, "kyouha");
    engine.process_key(&press_shift('A'));
    type_str(&mut engine, "bc");
    assert_eq!(engine.preedit().unwrap().text(), "きょうはAbc");

    // The cycle starts at 小文字, so the typed 'A' is lowercased first.
    engine.process_key(&press_key(Keysym::F10));
    assert_preedit(&engine, "kyouhaabc");

    // The cycle works on the mixed key sequence too.
    engine.process_key(&press_key(Keysym::F10));
    assert_preedit(&engine, "KYOUHAABC");
    engine.process_key(&press_key(Keysym::F10));
    assert_preedit(&engine, "Kyouhaabc");
}

#[test]
fn test_full_width_then_half_width_restarts_cycle() {
    let mut engine = InputMethodEngine::new();
    type_str(&mut engine, "nihongo");

    engine.process_key(&press_key(Keysym::F9));
    assert_preedit(&engine, "ｎｉｈｏｎｇｏ");
    engine.process_key(&press_key(Keysym::F10));
    assert_preedit(&engine, "nihongo");
}

// --- Empty state: keys pass through ---

#[test]
fn test_transliteration_keys_do_nothing_when_empty() {
    let mut engine = InputMethodEngine::new();
    for key in [
        press_key(Keysym::F6),
        press_key(Keysym::F7),
        press_key(Keysym::F8),
        press_key(Keysym::F9),
        press_key(Keysym::F10),
        press_ctrl(Keysym::KEY_J),
        press_ctrl(Keysym::KEY_K),
        press_ctrl(Keysym::SEMICOLON),
        press_ctrl(Keysym::KEY_L),
        press_ctrl(Keysym::COLON),
        press_ctrl(Keysym::APOSTROPHE),
    ] {
        let result = engine.process_key(&key);
        assert!(!result.consumed);
        assert!(matches!(engine.state(), InputState::Empty));
    }
}

#[test]
fn test_pending_romaji_flushes_before_transliteration() {
    let mut engine = InputMethodEngine::new();
    type_str(&mut engine, "nihong");

    // 'g' is still sitting in the romaji buffer; F6 must flush it first.
    engine.process_key(&press_key(Keysym::F6));
    assert_preedit(&engine, "にほんg");

    // Alphabet forms reproduce the full typed sequence including 'g'.
    engine.process_key(&press_key(Keysym::F10));
    assert_preedit(&engine, "nihong");
}
