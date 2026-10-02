use super::*;

// --- Katakana Conversion Tests ---
//
// Pure hiragana→katakana mapping is covered by `karukan_engine::kana` tests;
// the cases here exercise the IM-side state-machine integration (katakana
// mode entry, baking, etc.).
//
// Ctrl+K no longer enters katakana mode — it now runs the 全角カタカナ
// transliteration (Google日本語入力 parity, see tests/transliteration.rs).
// The mode itself is still exercised via `enter_katakana_mode()`.

#[test]
fn test_enter_katakana_mode_converts_display() {
    let mut engine = InputMethodEngine::new();

    // Type "aiueo" -> "あいうえお"
    engine.process_key(&press('a'));
    engine.process_key(&press('i'));
    engine.process_key(&press('u'));
    engine.process_key(&press('e'));
    engine.process_key(&press('o'));
    assert_eq!(engine.preedit().unwrap().text(), "あいうえお");

    // Katakana mode -> preedit shows "アイウエオ"
    let result = engine.enter_katakana_mode();

    assert!(result.consumed);
    // Should NOT commit yet - just convert display
    let has_commit = result
        .actions
        .iter()
        .any(|a| matches!(a, EngineAction::Commit(_)));
    assert!(!has_commit, "Should NOT commit on entering katakana mode");

    // Preedit should show katakana
    assert_eq!(engine.preedit().unwrap().text(), "アイウエオ");
    assert!(matches!(engine.state(), InputState::Composing { .. }));
    assert!(
        engine.input_mode == InputMode::Katakana,
        "Should be in katakana mode"
    );

    // Now press Enter -> should commit as katakana
    let enter_result = engine.process_key(&press_key(Keysym::RETURN));
    let has_katakana_commit = enter_result
        .actions
        .iter()
        .any(|a| matches!(a, EngineAction::Commit(text) if text == "アイウエオ"));
    assert!(has_katakana_commit, "Should commit as katakana after Enter");
    assert!(matches!(engine.state(), InputState::Empty));
}

#[test]
fn test_ctrl_k_with_empty_input() {
    let mut engine = InputMethodEngine::new();

    // No input, Ctrl+K should do nothing harmful
    let result = engine.process_key(&press_ctrl(Keysym::KEY_K));

    // Should not crash, state should remain empty
    assert!(matches!(engine.state(), InputState::Empty));
    // No commit action with empty text
    let has_commit = result
        .actions
        .iter()
        .any(|a| matches!(a, EngineAction::Commit(_)));
    assert!(!has_commit);
}

#[test]
fn test_enter_katakana_mode_persists_across_input() {
    let mut engine = InputMethodEngine::new();

    // Type "aiueo" -> "あいうえお"
    engine.process_key(&press('a'));
    engine.process_key(&press('i'));
    engine.process_key(&press('u'));
    engine.process_key(&press('e'));
    engine.process_key(&press('o'));
    assert_eq!(engine.preedit().unwrap().text(), "あいうえお");

    // Enter katakana mode -> preedit shows "アイウエオ"
    engine.enter_katakana_mode();
    assert_eq!(engine.preedit().unwrap().text(), "アイウエオ");
    assert!(
        engine.input_mode == InputMode::Katakana,
        "Should be in katakana mode"
    );

    // Type more → katakana mode persists (like alphabet mode)
    engine.process_key(&press('a'));
    assert!(
        engine.input_mode == InputMode::Katakana,
        "Katakana mode should persist across input"
    );
    // Preedit should show katakana for the new input too
    assert!(engine.preedit().unwrap().text().ends_with("ア"));
}

#[test]
fn test_katakana_baked_on_switch_to_alphabet() {
    let mut engine = InputMethodEngine::new();

    // Type "aiueo" → "あいうえお"
    engine.process_key(&press('a'));
    engine.process_key(&press('i'));
    engine.process_key(&press('u'));
    engine.process_key(&press('e'));
    engine.process_key(&press('o'));
    assert_eq!(engine.preedit().unwrap().text(), "あいうえお");

    // Katakana mode → displays "アイウエオ"
    engine.enter_katakana_mode();
    assert_eq!(engine.preedit().unwrap().text(), "アイウエオ");

    // Switch to alphabet mode via Shift+L → katakana should be baked in
    engine.process_key(&press_shift('L'));
    assert!(engine.input_mode == InputMode::Alphabet);
    // The katakana text should be preserved, not reverted to hiragana
    assert_eq!(engine.input_buf.text, "アイウエオL");

    // The first unshifted letter ends the session and returns to
    // Katakana mode: 'i' converts through romaji into い (displayed
    // as イ), then 'nu' becomes ヌ and 'x' stays pending romaji.
    engine.process_key(&press('i'));
    assert_eq!(engine.preedit().unwrap().text(), "アイウエオLイ");
    assert!(engine.input_mode == InputMode::Katakana);
    engine.process_key(&press('n'));
    engine.process_key(&press('u'));
    engine.process_key(&press('x'));
    assert_eq!(engine.preedit().unwrap().text(), "アイウエオLイヌx");
}

#[test]
fn test_katakana_mode_is_one_way() {
    let mut engine = InputMethodEngine::new();

    // Type "ai" → "あい"
    engine.process_key(&press('a'));
    engine.process_key(&press('i'));

    // Enter katakana mode
    engine.enter_katakana_mode();
    assert!(engine.input_mode == InputMode::Katakana);
    assert_eq!(engine.preedit().unwrap().text(), "アイ");

    // Entering again → still katakana mode (not a toggle)
    engine.enter_katakana_mode();
    assert!(engine.input_mode == InputMode::Katakana);
    assert_eq!(engine.preedit().unwrap().text(), "アイ");

    // Right Super → return to hiragana mode, katakana is baked in
    engine.process_key(&press_key(Keysym::SUPER_R));
    assert!(engine.input_mode == InputMode::Hiragana);
    assert_eq!(engine.input_buf.text, "アイ");
    assert_eq!(engine.preedit().unwrap().text(), "アイ");

    // New input in hiragana mode
    engine.process_key(&press('u'));
    assert_eq!(engine.preedit().unwrap().text(), "アイう");
}
