//! Tests for learning-backed Google IME style prediction and conversion.
//!
//! Tab/Down select visible predictions by default. Space starts ordinary
//! conversion, and `tab_skips_learning` restores the legacy Tab conversion path.

use karukan_engine::LearningCache;

use std::fs;
use std::time::SystemTime;

use super::*;

/// Engine seeded with a learning entry `reading → surface`, no kanji model.
/// We bypass `init.rs` (which gates learning on settings + file I/O) and just
/// inject a populated `LearningCache` directly — these tests assert the
/// build_conversion_candidates branching, not the load path.
fn engine_with_learned(reading: &str, surface: &str) -> InputMethodEngine {
    let mut engine = InputMethodEngine::new();
    engine.converters.kanji = None;
    let mut cache = LearningCache::new(100);
    cache.record(reading, surface);
    engine.learning = Some(cache);
    engine
}

fn engine_with_learning(entries: &[(&str, &str)]) -> InputMethodEngine {
    let mut engine = InputMethodEngine::new();
    engine.converters.kanji = None;
    let mut cache = LearningCache::new(100);
    for (reading, surface) in entries {
        cache.record(reading, surface);
    }
    engine.learning = Some(cache);
    engine
}

fn type_string(engine: &mut InputMethodEngine, text: &str) -> EngineResult {
    let mut result = EngineResult::default();
    for ch in text.chars() {
        result = engine.process_key(&press(ch));
    }
    result
}

fn shown_candidate_texts(result: &EngineResult) -> Vec<String> {
    result
        .actions
        .iter()
        .find_map(|action| match action {
            EngineAction::ShowCandidates(list) => Some(
                list.candidates()
                    .iter()
                    .map(|candidate| candidate.text.clone())
                    .collect(),
            ),
            _ => None,
        })
        .unwrap_or_default()
}

fn shown_candidate_cursor(result: &EngineResult) -> Option<usize> {
    result.actions.iter().find_map(|action| match action {
        EngineAction::ShowCandidates(list) => list.page_cursor(),
        _ => None,
    })
}

fn hides_candidates(result: &EngineResult) -> bool {
    result
        .actions
        .iter()
        .any(|action| matches!(action, EngineAction::HideCandidates))
}

fn committed_text(result: &EngineResult) -> Option<String> {
    result.actions.iter().find_map(|action| match action {
        EngineAction::Commit(text) => Some(text.clone()),
        _ => None,
    })
}

fn create_user_dict_file(dir: &std::path::Path, filename: &str, reading: &str, surface: &str) {
    let path = dir.join(filename);
    fs::write(
        path,
        format!("{reading}\t{surface}\t名詞\tgtype feedback\n"),
    )
    .unwrap();
}

fn candidate_texts(candidates: &[Candidate]) -> Vec<String> {
    candidates.iter().map(|c| c.text.clone()).collect()
}

#[test]
fn build_candidates_includes_learning_when_not_skipped() {
    let mut engine = engine_with_learned("あい", "藍");

    let texts: Vec<String> = engine
        .build_conversion_candidates("あい", 9, false)
        .into_iter()
        .map(|c| c.text)
        .collect();

    assert!(
        texts.contains(&"藍".to_string()),
        "Space path (skip_learning=false) should surface learned `藍`, got {:?}",
        texts,
    );
}

#[test]
fn build_candidates_omits_learning_when_skipped() {
    let mut engine = engine_with_learned("あい", "藍");

    let texts: Vec<String> = engine
        .build_conversion_candidates("あい", 9, true)
        .into_iter()
        .map(|c| c.text)
        .collect();

    assert!(
        !texts.contains(&"藍".to_string()),
        "Tab path (skip_learning=true) must drop learned `藍`, got {:?}",
        texts,
    );
}

#[test]
fn tab_key_skips_learning_in_composing() {
    // End-to-end: type the reading, press Tab → learned candidate is gone.
    let mut engine = engine_with_learned("あい", "藍");
    engine.config.tab_skips_learning = true;

    engine.process_key(&press('a'));
    engine.process_key(&press('i'));
    assert_eq!(engine.input_buf.text, "あい");

    let result = engine.process_key(&press_key(Keysym::TAB));
    assert!(result.consumed);
    assert!(matches!(engine.state(), InputState::Conversion { .. }));

    let texts: Vec<String> = engine
        .state()
        .candidates()
        .unwrap()
        .candidates()
        .iter()
        .map(|c| c.text.clone())
        .collect();
    assert!(
        !texts.contains(&"藍".to_string()),
        "Tab must skip the learned `藍` candidate, got {:?}",
        texts,
    );
}

#[test]
fn space_key_keeps_learning_in_composing() {
    // Counterpart to tab_key_skips_learning_in_composing: Space stays on the
    // learning-included path so the default UX is unchanged.
    let mut engine = engine_with_learned("あい", "藍");

    engine.process_key(&press('a'));
    engine.process_key(&press('i'));

    let result = engine.process_key(&press_key(Keysym::SPACE));
    assert!(result.consumed);
    assert!(matches!(engine.state(), InputState::Conversion { .. }));

    let texts: Vec<String> = engine
        .state()
        .candidates()
        .unwrap()
        .candidates()
        .iter()
        .map(|c| c.text.clone())
        .collect();
    assert!(
        texts.contains(&"藍".to_string()),
        "Space must surface learned `藍`, got {:?}",
        texts,
    );
}

#[test]
fn prediction_window_is_unselected_then_tab_enter_commits_visible_learning_prefix() {
    let mut engine = engine_with_learning(&[("よろしくおねがいします", "よろしくお願いします")]);

    let result = type_string(&mut engine, "yoroshiku");
    assert_eq!(engine.input_buf.text, "よろしく");
    assert_eq!(shown_candidate_cursor(&result), None);
    assert_eq!(shown_candidate_texts(&result), vec!["よろしくお願いします"]);

    let result = engine.process_key(&press_key(Keysym::TAB));
    assert!(result.consumed);
    assert!(matches!(engine.state(), InputState::Conversion { .. }));
    assert_eq!(shown_candidate_cursor(&result), Some(0));

    let result = engine.process_key(&press_key(Keysym::RETURN));
    assert_eq!(
        committed_text(&result).as_deref(),
        Some("よろしくお願いします")
    );
    assert!(
        engine
            .learning
            .as_ref()
            .unwrap()
            .lookup("よろしくおねがいします")
            .iter()
            .any(|(surface, _)| surface == "よろしくお願いします"),
        "choosing a prediction should be learned"
    );
}

#[test]
fn prediction_digit_commits_visible_candidate_immediately() {
    let mut engine = engine_with_learning(&[("よろしくおねがいします", "よろしくお願いします")]);
    let result = type_string(&mut engine, "yoroshiku");
    assert_eq!(shown_candidate_texts(&result), vec!["よろしくお願いします"]);

    let result = engine.process_key(&press('1'));
    assert_eq!(
        committed_text(&result).as_deref(),
        Some("よろしくお願いします")
    );
    assert!(matches!(engine.state(), InputState::Empty));
}

#[test]
fn user_dictionary_candidates_reload_after_file_change() {
    let temp_dir = tempfile::tempdir().unwrap();
    let mut engine = InputMethodEngine::new();

    create_user_dict_file(temp_dir.path(), "a_initial.tsv", "あんそ", "暗所");
    engine.init_user_dictionaries_with_dir(temp_dir.path());
    assert!(engine.dicts.user.is_some(), "user dict should be loaded");
    assert!(
        engine
            .dicts
            .user
            .as_ref()
            .and_then(|dict| dict.exact_match_search("あんそ"))
            .is_some(),
        "exact match should exist for あんそ"
    );
    engine.dicts.user_dict_last_checked = Some(SystemTime::now());

    let before = candidate_texts(&engine.build_prediction_candidates("あんそ"));
    assert_eq!(before, vec!["暗所"]);

    create_user_dict_file(temp_dir.path(), "z_added.tsv", "あんそ", "温泉");
    engine.refresh_user_dictionaries(Some(temp_dir.path()), true);
    engine.dicts.user_dict_last_checked = Some(SystemTime::now());

    let after = candidate_texts(&engine.build_prediction_candidates("あんそ"));
    assert_eq!(after, vec!["暗所", "温泉"]);
}

#[test]
fn tab_with_no_prediction_keeps_composing_unchanged() {
    let mut engine = engine_with_learning(&[]);
    let result = type_string(&mut engine, "ato");
    assert_eq!(engine.input_buf.text, "あと");
    assert!(hides_candidates(&result));
    assert!(shown_candidate_texts(&result).is_empty());

    let result = engine.process_key(&press_key(Keysym::TAB));
    assert!(result.consumed);
    assert!(result.actions.is_empty());
    assert_eq!(engine.input_buf.text, "あと");
    assert!(matches!(engine.state(), InputState::Composing { .. }));
}

#[test]
fn digit_with_no_prediction_continues_composing_input() {
    let mut engine = engine_with_learning(&[]);
    type_string(&mut engine, "ato");

    let result = engine.process_key(&press('1'));
    assert!(result.consumed);
    assert_eq!(committed_text(&result), None);
    assert_eq!(engine.input_buf.text, "あと1");
    assert!(matches!(engine.state(), InputState::Composing { .. }));
}

#[test]
fn prediction_ctrl_n_and_ctrl_p_move_selection_without_recomputing() {
    let mut engine = engine_with_learning(&[
        ("よろしくおねがいします", "よろしくお願いします"),
        ("よろしくどうぞ", "よろしくどうぞ"),
    ]);
    let result = type_string(&mut engine, "yoroshiku");
    let visible = shown_candidate_texts(&result);
    assert_eq!(visible.len(), 2);

    let result = engine.process_key(&press_key(Keysym::TAB));
    assert_eq!(shown_candidate_cursor(&result), Some(0));

    let result = engine.process_key(&press_ctrl(Keysym::KEY_N));
    assert_eq!(shown_candidate_cursor(&result), Some(1));

    let result = engine.process_key(&press_ctrl(Keysym::KEY_P));
    assert_eq!(shown_candidate_cursor(&result), Some(0));

    let result = engine.process_key(&press_key(Keysym::RETURN));
    assert_eq!(committed_text(&result), visible.first().cloned());
}

#[test]
fn prediction_window_excludes_identity_learning_entry() {
    let mut engine = engine_with_learning(&[("あと", "あと")]);
    let result = type_string(&mut engine, "ato");

    assert_eq!(engine.input_buf.text, "あと");
    assert!(
        !shown_candidate_texts(&result)
            .iter()
            .any(|text| text == "あと"),
        "identity learning entry must not appear in prediction window"
    );
}

#[test]
fn unchanged_enter_does_not_record_learning() {
    let mut engine = engine_with_learning(&[]);
    type_string(&mut engine, "ato");

    let result = engine.process_key(&press_key(Keysym::RETURN));
    assert_eq!(committed_text(&result).as_deref(), Some("あと"));
    assert!(
        engine.learning.as_ref().unwrap().lookup("あと").is_empty(),
        "plain hiragana Enter must not create あと→あと learning pollution"
    );
}

#[test]
fn space_conversion_excludes_learning_prefix_predictions() {
    let mut engine = engine_with_learning(&[("よろしくおねがいします", "よろしくお願いします")]);
    type_string(&mut engine, "yoroshiku");

    let result = engine.process_key(&press_key(Keysym::SPACE));
    assert!(result.consumed);
    let candidates: Vec<String> = engine
        .state()
        .candidates()
        .unwrap()
        .candidates()
        .iter()
        .map(|candidate| candidate.text.clone())
        .collect();
    assert!(
        !candidates.iter().any(|text| text == "よろしくお願いします"),
        "Space conversion must not include longer-reading learning predictions: {:?}",
        candidates
    );
}
