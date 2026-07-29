use super::*;
use crate::core::engine::conversion_cache::ConversionResultKey;
use karukan_engine::LearningCache;

#[test]
fn space_after_escape_reuses_nine_candidate_model_result() {
    let mut config = EngineConfig::default();
    config.num_candidates = 9;
    let mut engine = InputMethodEngine::with_config(config);
    let key = ConversionResultKey {
        reading: "あい".to_string(),
        left_context: String::new(),
        num_candidates: 9,
    };
    engine.conversion_result_cache.insert(
        key,
        vec!["P1_CACHE_SENTINEL".to_string()],
        ConversionStrategy::MainModelOnly,
        "cached-main".to_string(),
    );

    engine.process_key(&press('a'));
    engine.process_key(&press('i'));
    engine.process_key(&press_key(Keysym::SPACE));
    assert!(
        engine
            .candidates()
            .unwrap()
            .candidates()
            .iter()
            .any(|candidate| candidate.text == "P1_CACHE_SENTINEL")
    );

    engine.process_key(&press_key(Keysym::ESCAPE));
    engine.metrics.conversion_ms = 99;
    engine.metrics.model_name = "stale-model".to_string();
    engine.process_key(&press_key(Keysym::SPACE));

    assert!(
        engine
            .candidates()
            .unwrap()
            .candidates()
            .iter()
            .any(|candidate| candidate.text == "P1_CACHE_SENTINEL")
    );
    assert_eq!(engine.metrics.conversion_ms, 0);
    assert_eq!(engine.metrics.model_name, "cached-main");
}

#[test]
fn cache_hit_rebuilds_learning_candidates_in_current_priority_order() {
    let mut engine = InputMethodEngine::new();
    let key = ConversionResultKey {
        reading: "あい".to_string(),
        left_context: String::new(),
        num_candidates: 9,
    };
    engine.conversion_result_cache.insert(
        key,
        vec!["モデル候補".to_string()],
        ConversionStrategy::MainModelOnly,
        "cached-main".to_string(),
    );
    engine.learning = Some(LearningCache::new(100));

    let before = engine.build_conversion_candidates("あい", 9, false);
    assert!(
        before
            .iter()
            .any(|candidate| candidate.text == "モデル候補")
    );
    assert!(!before.iter().any(|candidate| candidate.text == "学習候補"));

    engine.learning.as_mut().unwrap().record("あい", "学習候補");
    let after = engine.build_conversion_candidates("あい", 9, false);

    assert_eq!(
        after.first().map(|candidate| candidate.text.as_str()),
        Some("学習候補")
    );
    assert!(after.iter().any(|candidate| candidate.text == "モデル候補"));
}

#[test]
fn cache_hit_restores_model_name_and_preserves_adaptive_flag() {
    let mut engine = InputMethodEngine::new();
    let key = ConversionResultKey {
        reading: "あい".to_string(),
        left_context: String::new(),
        num_candidates: 9,
    };
    engine.conversion_result_cache.insert(
        key,
        vec!["愛".to_string()],
        ConversionStrategy::MainModelOnly,
        "cached-main".to_string(),
    );
    engine.metrics.adaptive_use_light_model = true;
    engine.metrics.conversion_ms = 99;
    engine.metrics.model_name = "stale-model".to_string();

    let candidates = engine.run_kana_kanji_conversion("あい", "", 9);

    assert_eq!(candidates, vec!["愛"]);
    assert_eq!(engine.metrics.conversion_ms, 0);
    assert_eq!(engine.metrics.model_name, "cached-main");
    assert!(engine.metrics.adaptive_use_light_model);
}

#[test]
fn single_candidate_conversion_does_not_read_explicit_conversion_cache() {
    let mut engine = InputMethodEngine::new();
    let key = ConversionResultKey {
        reading: "あい".to_string(),
        left_context: String::new(),
        num_candidates: 1,
    };
    engine.conversion_result_cache.insert(
        key,
        vec!["cached-live-result".to_string()],
        ConversionStrategy::MainModelOnly,
        "cached-main".to_string(),
    );

    let candidates = engine.run_kana_kanji_conversion("あい", "", 1);

    assert!(candidates.is_empty());
}

#[test]
fn unavailable_model_does_not_create_completed_conversion_cache_entry() {
    let mut engine = InputMethodEngine::new();
    let key = ConversionResultKey {
        reading: "あい".to_string(),
        left_context: String::new(),
        num_candidates: 9,
    };

    let candidates = engine.run_kana_kanji_conversion("あい", "", 9);

    assert!(candidates.is_empty());
    assert_eq!(
        engine
            .conversion_result_cache
            .get(&key, &ConversionStrategy::MainModelOnly),
        None
    );
}

#[test]
fn test_conversion_char_commits_and_continues() {
    let mut engine = InputMethodEngine::new();

    // Type "あい" and enter conversion
    engine.process_key(&press('a'));
    engine.process_key(&press('i'));
    engine.process_key(&press_key(Keysym::SPACE));
    assert!(matches!(engine.state(), InputState::Conversion { .. }));

    // Type 'k' during conversion → should commit candidate and start new input
    let result = engine.process_key(&press('k'));
    assert!(result.consumed);

    // Should have committed the conversion
    let has_commit = result
        .actions
        .iter()
        .any(|a| matches!(a, EngineAction::Commit(_)));
    assert!(has_commit, "Should have a commit action");

    // Should now be in Composing with 'k' in preedit
    assert!(matches!(engine.state(), InputState::Composing { .. }));
    assert_eq!(engine.preedit().unwrap().text(), "k");
}

#[test]
fn test_conversion_char_commits_and_continues_romaji() {
    let mut engine = InputMethodEngine::new();

    // Type "あ" and enter conversion
    engine.process_key(&press('a'));
    engine.process_key(&press_key(Keysym::SPACE));
    assert!(matches!(engine.state(), InputState::Conversion { .. }));

    // Type 'k', 'a' → commits conversion, then starts "か"
    engine.process_key(&press('k'));
    assert!(matches!(engine.state(), InputState::Composing { .. }));
    assert_eq!(engine.preedit().unwrap().text(), "k");

    engine.process_key(&press('a'));
    assert_eq!(engine.preedit().unwrap().text(), "か");
}

#[test]
fn test_alphabet_mode_space_inserts_literal_space() {
    let mut engine = InputMethodEngine::new();

    // Enter alphabet mode via Shift+N
    engine.process_key(&press_shift('N'));
    assert!(engine.input_mode == InputMode::Alphabet);

    // Type "ew"
    engine.process_key(&press('e'));
    engine.process_key(&press('w'));
    assert_eq!(engine.preedit().unwrap().text(), "New");

    // Space → should insert literal space, NOT start conversion
    engine.process_key(&press_key(Keysym::SPACE));
    assert!(matches!(engine.state(), InputState::Composing { .. }));
    assert_eq!(engine.preedit().unwrap().text(), "New ");

    // Type "york"
    engine.process_key(&press('y'));
    engine.process_key(&press('o'));
    engine.process_key(&press('r'));
    engine.process_key(&press('k'));
    assert_eq!(engine.preedit().unwrap().text(), "New york");
}

#[test]
fn shift_tab_moves_to_previous_conversion_candidate() {
    let mut engine = InputMethodEngine::new();

    engine.process_key(&press('a'));
    engine.process_key(&press('i'));
    engine.process_key(&press_key(Keysym::SPACE));

    let first = engine.preedit().unwrap().text().to_string();
    engine.process_key(&press_key(Keysym::TAB));
    let second = engine.preedit().unwrap().text().to_string();
    assert_ne!(second, first);

    engine.process_key(&press_shift_key(Keysym::TAB));
    assert_eq!(engine.preedit().unwrap().text(), first);
}

#[test]
fn plain_arrows_are_consumed_during_conversion() {
    let mut engine = InputMethodEngine::new();

    engine.process_key(&press('a'));
    engine.process_key(&press('i'));
    engine.process_key(&press_key(Keysym::SPACE));
    let first = engine.preedit().unwrap().text().to_string();

    let left = engine.process_key(&press_key(Keysym::LEFT));
    assert!(left.consumed);
    assert!(left.actions.is_empty());
    assert_eq!(engine.preedit().unwrap().text(), first);

    let right = engine.process_key(&press_key(Keysym::RIGHT));
    assert!(right.consumed);
    assert!(right.actions.is_empty());
    assert_eq!(engine.preedit().unwrap().text(), first);
}

#[test]
fn shift_arrows_resize_conversion_target() {
    let mut engine = InputMethodEngine::new();

    engine.process_key(&press('a'));
    engine.process_key(&press('i'));
    engine.process_key(&press('u'));
    engine.process_key(&press_key(Keysym::SPACE));
    assert!(matches!(engine.state(), InputState::Conversion { .. }));

    engine.process_key(&press_shift_key(Keysym::LEFT));
    let preedit = engine.preedit().unwrap();
    let resized_text = preedit.text().to_string();
    assert!(resized_text.ends_with('う'));
    assert_eq!(preedit.caret(), resized_text.chars().count() - 1);
    assert_eq!(preedit.attributes()[0].attr_type, AttributeType::Highlight);
    assert_eq!(preedit.attributes()[0].start, 0);
    assert_eq!(preedit.attributes()[0].end, preedit.caret());
    assert_eq!(preedit.attributes()[1].attr_type, AttributeType::Underline);
    assert_eq!(preedit.attributes()[1].start, preedit.caret());
    assert_eq!(preedit.attributes()[1].end, resized_text.chars().count());

    engine.process_key(&press_shift_key(Keysym::RIGHT));
    let preedit = engine.preedit().unwrap();
    assert_eq!(preedit.text(), "あいう");
    assert_eq!(preedit.caret(), 3);
    assert_eq!(preedit.attributes().len(), 1);
    assert_eq!(preedit.attributes()[0].attr_type, AttributeType::Highlight);
    assert_eq!(preedit.attributes()[0].start, 0);
    assert_eq!(preedit.attributes()[0].end, 3);
}

#[test]
fn committing_resized_conversion_keeps_remainder() {
    let mut engine = InputMethodEngine::new();

    engine.process_key(&press('a'));
    engine.process_key(&press('i'));
    engine.process_key(&press('u'));
    engine.process_key(&press_key(Keysym::SPACE));
    engine.process_key(&press_shift_key(Keysym::LEFT));
    let expected_commit = engine.preedit().unwrap().text().to_string();

    let result = engine.process_key(&press_key(Keysym::RETURN));
    assert!(result.consumed);
    assert!(
        result
            .actions
            .iter()
            .any(|a| matches!(a, EngineAction::Commit(text) if text == &expected_commit))
    );
}

#[test]
fn right_arrow_after_resize_moves_to_remainder_conversion() {
    let mut engine = InputMethodEngine::new();

    engine.process_key(&press('a'));
    engine.process_key(&press('i'));
    engine.process_key(&press('u'));
    engine.process_key(&press('e'));
    engine.process_key(&press_key(Keysym::SPACE));
    engine.process_key(&press_shift_key(Keysym::LEFT));
    engine.process_key(&press_shift_key(Keysym::LEFT));

    let preedit = engine.preedit().unwrap().text().to_string();
    let caret = engine.preedit().unwrap().caret();
    let selected_text = preedit.chars().take(caret).collect::<String>();
    let remainder = preedit.chars().skip(caret).collect::<String>();
    assert_eq!(remainder, "うえ");

    let result = engine.process_key(&press_key(Keysym::RIGHT));
    assert!(result.consumed);
    assert!(
        !result
            .actions
            .iter()
            .any(|a| matches!(a, EngineAction::Commit(_)))
    );
    assert!(matches!(engine.state(), InputState::Conversion { .. }));
    assert!(engine.preedit().unwrap().text().starts_with(&selected_text));
    assert!(
        engine
            .candidates()
            .unwrap()
            .candidates()
            .iter()
            .any(|candidate| candidate.reading.as_deref() == Some("うえ"))
    );

    let result = engine.process_key(&press_key(Keysym::LEFT));
    assert!(result.consumed);
    assert!(
        !result
            .actions
            .iter()
            .any(|a| matches!(a, EngineAction::Commit(_)))
    );
    assert!(matches!(engine.state(), InputState::Conversion { .. }));
    assert_eq!(engine.preedit().unwrap().text(), preedit);
    assert!(
        engine
            .candidates()
            .unwrap()
            .candidates()
            .iter()
            .any(|candidate| candidate.reading.as_deref() == Some("あい"))
    );
}

#[test]
fn commit_after_right_arrow_conversion_includes_previous_segment() {
    let mut engine = InputMethodEngine::new();

    engine.process_key(&press('a'));
    engine.process_key(&press('i'));
    engine.process_key(&press('u'));
    engine.process_key(&press('e'));
    engine.process_key(&press_key(Keysym::SPACE));
    engine.process_key(&press_shift_key(Keysym::LEFT));
    engine.process_key(&press_shift_key(Keysym::LEFT));

    engine.process_key(&press_key(Keysym::RIGHT));
    let after_advance = engine.preedit().unwrap().text().to_string();

    let result = engine.process_key(&press_key(Keysym::RETURN));
    assert!(result.consumed);
    assert!(
        result
            .actions
            .iter()
            .any(|a| matches!(a, EngineAction::Commit(text) if text == &after_advance))
    );
}
