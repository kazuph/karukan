use super::*;

// --- Alphabet Mode Tests ---

#[test]
fn test_shift_alone_does_not_toggle_mode() {
    let mut engine = InputMethodEngine::new();
    assert!(engine.input_mode != InputMode::Alphabet);

    // Shift press alone should NOT toggle mode
    let result = engine.process_key(&press_key(Keysym::SHIFT_L));
    assert!(!result.consumed);
    assert!(engine.input_mode != InputMode::Alphabet);

    // Shift release is a no-op
    let result = engine.process_key(&release_key(Keysym::SHIFT_L));
    assert!(!result.consumed);
    assert!(engine.input_mode != InputMode::Alphabet);
}

#[test]
fn test_shift_letter_enters_alphabet_mode() {
    let mut engine = InputMethodEngine::new();
    assert!(engine.input_mode != InputMode::Alphabet);

    // Shift+A → enters alphabet mode and inputs 'A'
    engine.process_key(&press_shift('A'));
    assert!(engine.input_mode == InputMode::Alphabet);
    assert!(matches!(engine.state(), InputState::Composing { .. }));
    assert_eq!(engine.preedit().unwrap().text(), "A");

    // Release shift → no-op
    engine.process_key(&release_key(Keysym::SHIFT_L));
    assert!(engine.input_mode == InputMode::Alphabet); // Still in alphabet mode
}

#[test]
fn test_shift_letter_fcitx5_lowercase_keysym() {
    // fcitx5 sends lowercase keysym 'a' (0x0061) with shift modifier flag
    let mut engine = InputMethodEngine::new();
    assert!(engine.input_mode != InputMode::Alphabet);

    // fcitx5 sends keysym='a' (lowercase!) with modifiers.shift_key=true
    // This should enter alphabet mode and input uppercase 'A'
    let event = KeyEvent::new(
        Keysym(0x0061), // lowercase 'a'
        KeyModifiers::new().with_shift(true),
        true,
    );
    engine.process_key(&event);
    assert!(engine.input_mode == InputMode::Alphabet);
    assert!(matches!(engine.state(), InputState::Composing { .. }));
    // Should be uppercase 'A' in preedit
    assert_eq!(engine.preedit().unwrap().text(), "A");
}

#[test]
fn test_shift_letter_in_hiragana_enters_alphabet_and_uppercase() {
    let mut engine = InputMethodEngine::new();

    // Type some hiragana first
    engine.process_key(&press('a'));
    assert_eq!(engine.preedit().unwrap().text(), "あ");
    assert!(engine.input_mode != InputMode::Alphabet);

    // Shift press
    engine.process_key(&press_key(Keysym::SHIFT_L));

    // Shift+a (fcitx5 sends lowercase keysym)
    let event = KeyEvent::new(Keysym(0x0061), KeyModifiers::new().with_shift(true), true);
    engine.process_key(&event);
    assert!(engine.input_mode == InputMode::Alphabet);
    assert_eq!(engine.preedit().unwrap().text(), "あA");
}

#[test]
fn test_uppercase_keysym_without_shift_flag_enters_alphabet() {
    // fcitx5 may resolve Shift into the keysym, sending 'A' (0x0041) without
    // the shift modifier flag. This must still enter alphabet mode.
    let mut engine = InputMethodEngine::new();
    assert!(engine.input_mode != InputMode::Alphabet);

    // Empty state: uppercase keysym without shift flag
    let event = KeyEvent::new(
        Keysym(0x0041), // uppercase 'A'
        KeyModifiers::new(),
        true,
    );
    engine.process_key(&event);
    assert!(
        engine.input_mode == InputMode::Alphabet,
        "Uppercase keysym should enter alphabet mode even without shift flag"
    );
    assert_eq!(engine.preedit().unwrap().text(), "A");
}

#[test]
fn test_uppercase_keysym_without_shift_flag_composing() {
    // Same as above but in Composing state (hiragana already entered)
    let mut engine = InputMethodEngine::new();

    // Type hiragana first
    engine.process_key(&press('a'));
    assert_eq!(engine.preedit().unwrap().text(), "あ");
    assert!(engine.input_mode != InputMode::Alphabet);

    // Uppercase keysym without shift flag
    let event = KeyEvent::new(
        Keysym(0x0041), // uppercase 'A'
        KeyModifiers::new(),
        true,
    );
    engine.process_key(&event);
    assert!(
        engine.input_mode == InputMode::Alphabet,
        "Uppercase keysym should enter alphabet mode during composing"
    );
    assert_eq!(engine.preedit().unwrap().text(), "あA");
}

#[test]
fn test_shift_symbol_stays_in_hiragana_mode() {
    // Shift+symbol should NOT enter alphabet mode (only Shift+letter does)
    let mut engine = InputMethodEngine::new();
    assert!(engine.input_mode != InputMode::Alphabet);

    // '!' with shift modifier → stays in hiragana mode
    let event = KeyEvent::new(
        Keysym(0x0021), // '!'
        KeyModifiers::new().with_shift(true),
        true,
    );
    engine.process_key(&event);
    assert!(engine.input_mode != InputMode::Alphabet);
}

#[test]
fn test_shift_digit_stays_in_hiragana_mode() {
    // Shift+digit should NOT enter alphabet mode (only Shift+letter does)
    let mut engine = InputMethodEngine::new();
    assert!(engine.input_mode != InputMode::Alphabet);

    // '2' with shift modifier → stays in hiragana mode
    let event = KeyEvent::new(
        Keysym(0x0032), // '2'
        KeyModifiers::new().with_shift(true),
        true,
    );
    engine.process_key(&event);
    assert!(engine.input_mode != InputMode::Alphabet);
}

#[test]
fn test_alphabet_mode_uppercase_with_shift() {
    let mut engine = InputMethodEngine::new();

    // Shift+A → enters alphabet mode and inputs 'A'
    engine.process_key(&press_shift('A'));
    assert!(engine.input_mode == InputMode::Alphabet);

    // Type lowercase 'a' → ends the session and becomes あ
    engine.process_key(&press('a'));
    assert_eq!(engine.preedit().unwrap().text(), "Aあ");
    assert!(engine.input_mode == InputMode::Hiragana);

    // Shift+a → reopens the session: uppercase 'A' in alphabet mode
    let event = KeyEvent::new(
        Keysym(0x0061), // lowercase keysym
        KeyModifiers::new().with_shift(true),
        true,
    );
    engine.process_key(&event);
    assert!(engine.input_mode == InputMode::Alphabet);
    assert_eq!(engine.preedit().unwrap().text(), "AあA");
}

#[test]
fn test_alphabet_mode_direct_input() {
    let mut engine = InputMethodEngine::new();

    // Shift+A → enters alphabet mode and inputs 'A'
    engine.process_key(&press_shift('A'));
    assert!(engine.input_mode == InputMode::Alphabet);
    assert!(matches!(engine.state(), InputState::Composing { .. }));
    assert_eq!(engine.preedit().unwrap().text(), "A");

    // Type 'b' → ends the session; 'b' stays pending romaji → "Ab"
    engine.process_key(&press('b'));
    assert_eq!(engine.preedit().unwrap().text(), "Ab");

    // Type 'c' → "bc" isn't a romaji start, so 'b' flushes through
    // literally and 'c' stays pending → "Abc"
    engine.process_key(&press('c'));
    assert_eq!(engine.preedit().unwrap().text(), "Abc");
}

#[test]
fn test_mixed_hiragana_alphabet_input() {
    let mut engine = InputMethodEngine::new();

    // Type hiragana first: "わたしは"
    engine.process_key(&press('w'));
    engine.process_key(&press('a'));
    engine.process_key(&press('t'));
    engine.process_key(&press('a'));
    engine.process_key(&press('s'));
    engine.process_key(&press('i'));
    engine.process_key(&press('h'));
    engine.process_key(&press('a'));
    assert_eq!(engine.preedit().unwrap().text(), "わたしは");
    assert!(engine.input_mode != InputMode::Alphabet);

    // Shift+L → enters alphabet mode, inputs 'L'
    // fcitx5 sends lowercase keysym with shift flag
    let event = KeyEvent::new(
        Keysym(0x006c), // lowercase 'l'
        KeyModifiers::new().with_shift(true),
        true,
    );
    engine.process_key(&event);
    assert!(engine.input_mode == InputMode::Alphabet);
    assert_eq!(engine.preedit().unwrap().text(), "わたしはL");

    // Unshifted letters end the session and convert as romaji: 'i'
    // becomes い (the 'L' stays a literal and seeds nothing), then 'nu'
    // becomes ぬ and 'x' stays pending.
    engine.process_key(&press('i'));
    assert_eq!(engine.preedit().unwrap().text(), "わたしはLい");
    assert!(engine.input_mode == InputMode::Hiragana);
    engine.process_key(&press('n'));
    engine.process_key(&press('u'));
    engine.process_key(&press('x'));
    assert_eq!(engine.preedit().unwrap().text(), "わたしはLいぬx");
}

#[test]
fn test_alphabet_mode_persists_across_commit() {
    let mut engine = InputMethodEngine::new();

    // Enter alphabet mode via Shift+H and commit the literals
    engine.process_key(&press_shift('H'));
    engine.process_key(&press_shift('I'));
    assert!(engine.input_mode == InputMode::Alphabet);
    assert_eq!(engine.preedit().unwrap().text(), "HI");

    engine.process_key(&press_key(Keysym::RETURN));
    assert!(matches!(engine.state(), InputState::Empty));

    // The session ended at commit: hiragana input is back
    assert!(engine.input_mode == InputMode::Hiragana);

    // New input is romaji again
    engine.process_key(&press('y'));
    engine.process_key(&press('a'));
    assert_eq!(engine.preedit().unwrap().text(), "や");
}

#[test]
fn test_alphabet_mode_cancel_clears_flags() {
    let mut engine = InputMethodEngine::new();

    // Enter alphabet mode via Shift+A, type more shifted letters, cancel
    engine.process_key(&press_shift('A'));
    engine.process_key(&press_shift('B'));

    engine.process_key(&press_key(Keysym::ESCAPE));
    assert!(matches!(engine.state(), InputState::Empty));
    // Cancel ends the session: back to hiragana, not alphabet
    assert!(engine.input_mode == InputMode::Hiragana);
}

#[test]
fn test_alphabet_mode_aux_text() {
    let mut engine = InputMethodEngine::new();

    // In hiragana mode, aux should show [あ]
    engine.process_key(&press('a'));
    let aux_hiragana = engine.format_aux_composing();
    assert!(aux_hiragana.starts_with("[あ]"));

    engine.process_key(&press_key(Keysym::ESCAPE));

    // Enter alphabet mode via Shift+A
    engine.process_key(&press_shift('A'));

    let aux_alpha = engine.format_aux_composing();
    assert!(aux_alpha.starts_with("[A]"));
}

#[test]
fn test_shift_right_alone_does_not_toggle() {
    let mut engine = InputMethodEngine::new();
    assert!(engine.input_mode != InputMode::Alphabet);

    // Right Shift alone should NOT toggle alphabet mode
    engine.process_key(&press_key(Keysym::SHIFT_R));
    assert!(engine.input_mode != InputMode::Alphabet);
}

#[test]
fn test_reset_clears_alphabet_mode() {
    let mut engine = InputMethodEngine::new();

    // Enter alphabet mode via Shift+A
    engine.process_key(&press_shift('A'));
    assert!(engine.input_mode == InputMode::Alphabet);

    engine.reset();
    assert!(engine.input_mode != InputMode::Alphabet);
}

// --- Shift+letter temporary session → kana return ---

#[test]
fn test_shift_alpha_unshifted_letters_resume_romaji() {
    // Shift+A, Shift+B stay uppercase literals; the first unshifted
    // letter ends the session and resumes romaji: 'k' buffers, 'a'
    // completes か.
    let mut engine = InputMethodEngine::new();
    engine.process_key(&press_shift('A'));
    engine.process_key(&press_shift('B'));
    assert_eq!(engine.preedit().unwrap().text(), "AB");
    assert!(engine.input_mode == InputMode::Alphabet);

    engine.process_key(&press('k'));
    assert_eq!(engine.preedit().unwrap().text(), "ABk");
    assert_eq!(engine.input_mode, InputMode::Hiragana);

    engine.process_key(&press('a'));
    assert_eq!(engine.preedit().unwrap().text(), "ABか");
}

#[test]
fn test_shift_alpha_letters_never_seed_romaji() {
    // Shifted letters are pure literals — their lowercase forms do NOT
    // feed romaji. Shift+T then "oukyou" converts "oukyou" alone:
    // 'o' ends the session and becomes お, so the result is Tおうきょう,
    // not Tとうきょう.
    let mut engine = InputMethodEngine::new();
    engine.process_key(&press_shift('T'));
    assert_eq!(engine.preedit().unwrap().text(), "T");

    for ch in "oukyou".chars() {
        engine.process_key(&press(ch));
    }
    assert_eq!(engine.preedit().unwrap().text(), "Tおうきょう");
    assert_eq!(engine.input_mode, InputMode::Hiragana);
}

#[test]
fn test_shift_alpha_consonant_followed_by_vowel() {
    // The shifted consonant doesn't carry over: Shift+K / Shift+N then
    // 'a' is just 'a' → あ, not か / な.
    let mut engine = InputMethodEngine::new();
    engine.process_key(&press_shift('K'));
    engine.process_key(&press('a'));
    assert_eq!(engine.preedit().unwrap().text(), "Kあ");
    assert_eq!(engine.input_mode, InputMode::Hiragana);

    let mut engine = InputMethodEngine::new();
    engine.process_key(&press_shift('N'));
    engine.process_key(&press('a'));
    assert_eq!(engine.preedit().unwrap().text(), "Nあ");
}

#[test]
fn test_shift_alpha_fcitx5_lowercase_keysym_resume() {
    // Same session end, but Shift+A arrives as lowercase keysym + shift
    // flag the way fcitx5 delivers it.
    let mut engine = InputMethodEngine::new();
    let shift_a = KeyEvent::new(
        Keysym(0x0061), // lowercase 'a'
        KeyModifiers::new().with_shift(true),
        true,
    );
    engine.process_key(&shift_a);
    let shift_b = KeyEvent::new(Keysym(0x0062), KeyModifiers::new().with_shift(true), true);
    engine.process_key(&shift_b);
    assert_eq!(engine.preedit().unwrap().text(), "AB");

    engine.process_key(&press('k'));
    engine.process_key(&press('a'));
    assert_eq!(engine.preedit().unwrap().text(), "ABか");
    assert_eq!(engine.input_mode, InputMode::Hiragana);
}

#[test]
fn test_shift_alpha_digits_and_symbols_stay_literal() {
    // Unshifted digits and symbols don't end the session — they insert
    // literally like any alphabet-mode input; the next unshifted letter
    // still returns to kana.
    let mut engine = InputMethodEngine::new();
    engine.process_key(&press_shift('A'));
    engine.process_key(&press_shift('B'));
    engine.process_key(&press('1'));
    engine.process_key(&press('!'));
    assert_eq!(engine.preedit().unwrap().text(), "AB1!");
    assert!(engine.input_mode == InputMode::Alphabet);

    engine.process_key(&press('k'));
    engine.process_key(&press('a'));
    assert_eq!(engine.preedit().unwrap().text(), "AB1!か");
}

#[test]
fn test_shift_alpha_backspace_then_unshifted_letter() {
    // Backspace removes just the literal; the session survives, so the
    // next unshifted letter still returns to kana input.
    let mut engine = InputMethodEngine::new();
    engine.process_key(&press_shift('K'));
    engine.process_key(&press_shift('Y'));
    assert_eq!(engine.preedit().unwrap().text(), "KY");

    engine.process_key(&press_key(Keysym::BACKSPACE));
    assert_eq!(engine.preedit().unwrap().text(), "K");

    engine.process_key(&press('a'));
    assert_eq!(engine.preedit().unwrap().text(), "Kあ");
}
