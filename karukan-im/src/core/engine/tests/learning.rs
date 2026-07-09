//! Tests for the learning cache and the Tab-skips-learning behavior.
//!
//! Space/Down: include learning candidates (default conversion).
//! Tab: skip learning candidates (lets users escape stale learned entries).

use karukan_engine::LearningCache;

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
fn adjust_learning_delete_removes_candidate() {
    let mut engine = engine_with_learned("あい", "藍");

    let result = engine.adjust_learning_candidate("あい", "藍", LearningAdjustment::Delete);
    assert!(result.consumed);

    let texts: Vec<String> = engine
        .lookup_learning_candidates("あい")
        .into_iter()
        .map(|c| c.text)
        .collect();
    assert!(
        !texts.contains(&"藍".to_string()),
        "Delete must remove learned `藍`, got {:?}",
        texts,
    );
}

#[test]
fn adjust_learning_promote_moves_candidate_to_top() {
    let mut engine = engine_with_learned("あい", "藍");
    engine.record_learning("あい", "愛");
    engine.record_learning("あい", "愛");

    let before: Vec<String> = engine
        .lookup_learning_candidates("あい")
        .into_iter()
        .map(|c| c.text)
        .collect();
    assert_eq!(before[0], "愛");

    let result = engine.adjust_learning_candidate("あい", "藍", LearningAdjustment::Promote);
    assert!(result.consumed);

    let after: Vec<String> = engine
        .lookup_learning_candidates("あい")
        .into_iter()
        .map(|c| c.text)
        .collect();
    assert_eq!(after[0], "藍");
}

#[test]
fn adjust_learning_demote_lowers_candidate_score() {
    let mut engine = engine_with_learned("あい", "藍");
    engine.record_learning("あい", "藍");

    let before = engine.learning.as_ref().unwrap().lookup("あい")[0].1;
    let result = engine.adjust_learning_candidate("あい", "藍", LearningAdjustment::Demote);
    assert!(result.consumed);
    let after = engine.learning.as_ref().unwrap().lookup("あい")[0].1;

    assert!(after < before);
}
