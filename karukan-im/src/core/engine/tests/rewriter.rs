//! Integration tests for the rewriter chain in conversion candidates.
//!
//! No kanji model is loaded — these tests exercise only the rewriter path,
//! so they verify behaviour that holds independently of model output.

use std::collections::HashSet;
use std::io::Write;

use karukan_engine::{Dictionary, LearningCache, RewriterChain};

use super::*;

// ---------- helpers ----------

/// Engine in Composing state with the kanji model explicitly disabled.
fn composing_engine(reading: &str) -> InputMethodEngine {
    let mut engine = InputMethodEngine::new();
    engine.input_buf.text = reading.to_string();
    engine.input_buf.cursor_pos = reading.chars().count();
    engine.state = InputState::Composing {
        preedit: Preedit::new(),
        romaji_buffer: String::new(),
    };
    engine.converters.kanji = None;
    engine
}

/// Run `build_conversion_candidates` and return just the candidate texts.
fn conversion_texts(reading: &str) -> Vec<String> {
    let mut engine = composing_engine(reading);
    engine
        .build_conversion_candidates(reading, 9, false)
        .into_iter()
        .map(|c| c.text)
        .collect()
}

/// Build a one-entry user dictionary as a temp JSON file and load it.
fn user_dict_with(reading: &str, surface: &str) -> Dictionary {
    let mut tmp = tempfile::NamedTempFile::new().unwrap();
    let json = format!(
        r#"[{{"reading":"{reading}","candidates":[{{"surface":"{surface}","score":1.0}}]}}]"#
    );
    tmp.write_all(json.as_bytes()).unwrap();
    tmp.flush().unwrap();
    Dictionary::build_from_json(tmp.path()).unwrap()
}

/// Drive the engine by typing a string of characters.
fn type_string(engine: &mut InputMethodEngine, s: &str) {
    for ch in s.chars() {
        engine.process_key(&press(ch));
    }
}

/// Texts in the conversion-state candidate list.
fn conversion_state_texts(engine: &InputMethodEngine) -> Vec<String> {
    engine
        .state()
        .candidates()
        .map(|cl| cl.candidates().iter().map(|c| c.text.clone()).collect())
        .unwrap_or_default()
}

/// Texts from the most recent ShowCandidates action (auto-suggest path).
fn auto_suggest_texts(result: &EngineResult) -> Vec<String> {
    result
        .actions
        .iter()
        .find_map(|a| match a {
            EngineAction::ShowCandidates(list) => {
                Some(list.candidates().iter().map(|c| c.text.clone()).collect())
            }
            _ => None,
        })
        .unwrap_or_default()
}

#[track_caller]
fn assert_contains(texts: &[String], expected: &str) {
    assert!(
        texts.iter().any(|t| t == expected),
        "expected `{expected}` in candidates, got: {texts:?}"
    );
}

#[track_caller]
fn assert_not_contains(texts: &[String], forbidden: &str) {
    assert!(
        !texts.iter().any(|t| t == forbidden),
        "`{forbidden}` should NOT be in candidates, got: {texts:?}"
    );
}

// ---------- half-width katakana variants ----------

#[test]
fn single_hiragana_emits_half_width_katakana() {
    assert_contains(&conversion_texts("あ"), "ｱ");
}

#[test]
fn hiragana_word_emits_half_width_katakana() {
    assert_contains(&conversion_texts("がっこう"), "ｶﾞｯｺｳ");
}

#[test]
fn plain_hiragana_word_is_not_wrapped_in_brackets() {
    let texts = conversion_texts("あいう");
    assert_not_contains(&texts, "「あいう」");
    assert_not_contains(&texts, "【あいう】");
}

// ---------- symbol variants ----------

#[test]
fn three_full_stops_emit_ellipsis_without_kanji_model() {
    // Regression: previously `build_conversion_candidates` early-returned
    // when init_kanji_converter failed, so symbol-only inputs lost rewriter
    // variants and `。。。` produced no `…`.
    assert_contains(&conversion_texts("。。。"), "…");
}

// ---------- rewriter scope (the headline regression) ----------

#[test]
fn rewriter_does_not_expand_dictionary_candidates() {
    // The rewriter must operate ONLY on what the user typed, not on
    // dictionary/model/fallback candidates derived from it.
    //
    // Setup: a user dict entry maps `てすと` → `,` (ASCII comma). The
    // SymbolRewriter has `,` → [`、`, `，`, `､`]. If the rewriter were
    // (wrongly) fed dictionary candidates, those three variants would
    // appear even though the user typed only hiragana.
    let mut engine = composing_engine("てすと");
    engine.dicts.user = Some(user_dict_with("てすと", ","));

    let texts: Vec<String> = engine
        .build_conversion_candidates("てすと", 9, false)
        .into_iter()
        .map(|c| c.text)
        .collect();

    assert_contains(&texts, ","); // sanity: dict entry survives
    for forbidden in ["、", "，", "､"] {
        assert_not_contains(&texts, forbidden);
    }
}

#[test]
fn rewriter_candidates_only_derive_from_user_input() {
    // Structural invariant: every Rewriter-source candidate must be a
    // rewrite of the typed reading. Guards against future regressions where
    // somebody re-introduces rewriting over dictionary/model/fallback entries.
    let mut engine = composing_engine("あ");
    let candidates = engine.build_conversion_candidates("あ", 9, false);

    let allowed: HashSet<String> = RewriterChain::default_chain()
        .rewrite_all(&["あ".to_string()])
        .into_iter()
        .map(|(text, _)| text)
        .collect();

    for c in &candidates {
        if c.source == CandidateSource::Rewriter {
            assert!(
                allowed.contains(&c.text),
                "Rewriter candidate `{}` is not a rewrite of input `あ` \
                 (allowed: {:?})",
                c.text,
                allowed
            );
        }
    }
}

// ---------- alphabet width / case variants ----------

#[test]
fn alphabet_input_emits_width_and_case_variants() {
    // Typing `abc` (e.g. in a passthrough or alphabet path) should expand to
    // the other three canonical forms.
    let texts = conversion_texts("abc");
    assert_contains(&texts, "ABC");
    assert_contains(&texts, "ａｂｃ");
    assert_contains(&texts, "ＡＢＣ");
}

#[test]
fn alphabet_variants_carry_width_case_descriptions() {
    let mut engine = composing_engine("abc");
    let candidates = engine.build_conversion_candidates("abc", 9, false);
    let upper = candidates.iter().find(|c| c.text == "ABC").unwrap();
    let full_lower = candidates.iter().find(|c| c.text == "ａｂｃ").unwrap();
    let full_upper = candidates.iter().find(|c| c.text == "ＡＢＣ").unwrap();
    assert_eq!(upper.description.as_deref(), Some("[半]英大文字"));
    assert_eq!(full_lower.description.as_deref(), Some("[全]英小文字"));
    assert_eq!(full_upper.description.as_deref(), Some("[全]英大文字"));
}

// ---------- description (per-candidate right-side comment) ----------

/// Find the candidate with text == `text` in the conversion-state list and
/// return its per-candidate `description` (mozc-style right-side comment).
fn description_for(engine: &InputMethodEngine, text: &str) -> Option<String> {
    engine.state().candidates().and_then(|cl| {
        cl.candidates()
            .iter()
            .find(|c| c.text == text)
            .and_then(|c| c.description.clone())
    })
}

#[test]
fn ellipsis_candidate_carries_three_dot_leader_description() {
    // When `。。。` expands via the rewriter, `…` should carry mozc's
    // description "三点リーダ" as its per-candidate description (not
    // duplicated into the aux text source-label slot).
    let mut engine = InputMethodEngine::new();
    type_string(&mut engine, "..");
    engine.process_key(&press('.'));
    engine.process_key(&press_key(Keysym::SPACE));

    assert_eq!(
        description_for(&engine, "…"),
        Some("三点リーダ".to_string())
    );
}

#[test]
fn typed_symbol_itself_is_annotated_via_global_lookup() {
    // The user typed `「` itself — it appears as a Fallback candidate
    // (not from the rewriter). The post-pass enrichment should still attach
    // mozc's description "始めかぎ括弧" because `「` is a known symbol.
    let mut engine = composing_engine("「");
    let candidates = engine.build_conversion_candidates("「", 9, false);

    let kagi = candidates.iter().find(|c| c.text == "「").unwrap();
    assert_eq!(
        kagi.description.as_deref(),
        Some("始めかぎ括弧"),
        "Fallback `「` should pick up `始めかぎ括弧` via symbol_description"
    );
}

#[test]
fn kagikakko_reading_emits_paired_brackets_in_conversion() {
    // End-to-end: typing the reading `かぎかっこ` and pressing Space should
    // surface `「」` and `『』` in the conversion candidate list (mozc
    // symbol.tsv via SymbolRewriter).
    let texts = conversion_texts("かぎかっこ");
    assert_contains(&texts, "「」");
    assert_contains(&texts, "『』");
}

#[test]
fn katakana_variants_carry_width_form_description() {
    // mozc-style width annotation: full-width katakana → `[全]カタカナ`,
    // half-width katakana → `[半]カタカナ`. The hiragana fallback also picks
    // up `[全]ひらがな` since hiragana is intrinsically full-width.
    let mut engine = composing_engine("あ");
    let candidates = engine.build_conversion_candidates("あ", 9, false);

    let hira = candidates.iter().find(|c| c.text == "あ").unwrap();
    assert_eq!(
        hira.description.as_deref(),
        Some("[全]ひらがな"),
        "hiragana fallback `あ` should be annotated as `[全]ひらがな`",
    );

    let full = candidates.iter().find(|c| c.text == "ア").unwrap();
    assert_eq!(
        full.description.as_deref(),
        Some("[全]カタカナ"),
        "full-width katakana variant `ア` should be annotated as `[全]カタカナ`",
    );

    let half = candidates.iter().find(|c| c.text == "ｱ").unwrap();
    assert_eq!(
        half.description.as_deref(),
        Some("[半]カタカナ"),
        "half-width katakana variant `ｱ` should be annotated as `[半]カタカナ`",
    );
}

// ---------- end-to-end key flow ----------

#[test]
fn typing_three_dots_emits_ellipsis_in_conversion_not_prediction_window() {
    // Regression: typing `.` `.` `.` should populate `。。。` and surface `…`
    // in the Space conversion candidate list. The Google IME style prediction
    // window is now reserved for learning/user-dictionary predictions.
    let mut engine = InputMethodEngine::new();
    type_string(&mut engine, "..");
    let final_result = engine.process_key(&press('.'));
    assert_eq!(engine.input_buf.text, "。。。");

    assert!(
        auto_suggest_texts(&final_result).is_empty(),
        "prediction window should not show rewriter-only candidates"
    );

    engine.process_key(&press_key(Keysym::SPACE));
    assert_contains(&conversion_state_texts(&engine), "…");
}

#[test]
fn typing_a_then_space_emits_half_width_katakana() {
    let mut engine = InputMethodEngine::new();
    type_string(&mut engine, "a");

    let result = engine.process_key(&press_key(Keysym::SPACE));
    assert!(result.consumed);
    assert_contains(&conversion_state_texts(&engine), "ｱ");
}

// ---------- explicit special conversions ----------

#[test]
fn today_conversion_precedes_literal_fallback() {
    let texts = conversion_texts("きょう");
    let date_index = texts
        .iter()
        .position(|text| text.contains('/') || text.contains("年"))
        .expect("きょう should produce a formatted date");
    let fallback_index = texts.iter().position(|text| text == "きょう").unwrap();
    assert!(date_index < CandidateList::DEFAULT_PAGE_SIZE);
    assert!(
        date_index < fallback_index,
        "special date must stay on the first page"
    );
}

#[test]
fn user_dictionary_stays_ahead_of_special_conversion() {
    let mut engine = composing_engine("きょう");
    engine.dicts.user = Some(user_dict_with("きょう", "利用者の今日"));

    let texts: Vec<String> = engine
        .build_conversion_candidates("きょう", 9, false)
        .into_iter()
        .map(|candidate| candidate.text)
        .collect();

    assert_eq!(texts.first().map(String::as_str), Some("利用者の今日"));
    assert!(texts.iter().any(|text| text.contains('/')));
}

#[test]
fn dynamic_special_conversion_is_not_learned_after_commit() {
    let mut engine = InputMethodEngine::new();
    engine.learning = Some(LearningCache::new(100));
    type_string(&mut engine, "kyou");

    engine.process_key(&press_key(Keysym::SPACE));
    let candidates = engine.state().candidates().unwrap();
    let first = candidates.candidates().first().unwrap();
    assert!(first.text.contains('/'));
    assert!(first.reading.is_none());

    let result = engine.select_candidate_on_page(0);
    assert!(
        result
            .actions
            .iter()
            .any(|action| matches!(action, EngineAction::Commit(text) if text.contains('/')))
    );
    assert!(
        engine
            .learning
            .as_ref()
            .unwrap()
            .lookup("きょう")
            .is_empty()
    );
}

#[test]
fn resized_dynamic_special_conversion_is_not_learned_after_commit() {
    let mut engine = InputMethodEngine::new();
    engine.learning = Some(LearningCache::new(100));
    type_string(&mut engine, "kyoua");

    engine.process_key(&press_key(Keysym::SPACE));
    engine.process_key(&press_shift_key(Keysym::LEFT));

    let candidates = engine.state().candidates().unwrap();
    let first = candidates.candidates().first().unwrap();
    assert!(first.text.contains('/'));
    assert!(first.reading.is_none());

    let result = engine.select_candidate_on_page(0);
    assert!(result.actions.iter().any(
        |action| matches!(action, EngineAction::Commit(text) if text.contains('/') && text.ends_with('あ'))
    ));
    assert!(
        engine
            .learning
            .as_ref()
            .unwrap()
            .lookup("きょう")
            .is_empty()
    );
}

#[test]
fn four_digits_emit_date_and_time_before_numeric_variants() {
    let texts = conversion_texts("1230");
    assert_contains(&texts, "12月30日");
    assert_contains(&texts, "12時30分");
    assert_contains(&texts, "12時半");

    let date_index = texts.iter().position(|text| text == "12月30日").unwrap();
    let fallback_index = texts.iter().position(|text| text == "1230").unwrap();
    assert!(date_index < fallback_index);

    assert_contains(&conversion_texts("645ねん"), "大化元年");
}

#[test]
fn calculator_uses_the_real_key_input_path() {
    let mut engine = InputMethodEngine::new();
    type_string(&mut engine, "100/8=");
    assert_eq!(engine.input_buf.text, "100・8=");

    engine.process_key(&press_key(Keysym::SPACE));
    assert_contains(&conversion_state_texts(&engine), "12.5");
}

#[test]
fn unicode_codepoint_converts_from_alphabet_mode_on_space() {
    let mut engine = InputMethodEngine::new();
    type_string(&mut engine, "U+611B");
    assert_eq!(engine.input_mode, InputMode::Alphabet);

    engine.process_key(&press_key(Keysym::SPACE));
    assert_contains(&conversion_state_texts(&engine), "愛");
}

#[test]
fn invalid_unicode_keeps_alphabet_space_behavior() {
    let mut engine = InputMethodEngine::new();
    type_string(&mut engine, "U+110000");
    let result = engine.process_key(&press_key(Keysym::SPACE));

    assert!(engine.state().candidates().is_none());
    assert_eq!(engine.input_buf.text, "U+110000 ");
    assert!(result.consumed);
}

#[test]
fn version_face_and_correction_candidates_are_available() {
    let version = conversion_texts("ばーじょん");
    assert!(version.iter().any(|text| text.starts_with("Karukan ")));

    assert_contains(&conversion_texts("にこにこ"), "(^^)");
    assert_contains(&conversion_texts("あぼがど"), "アボカド");
    assert_contains(&conversion_texts("しゅみれーしょん"), "シミュレーション");
}

#[test]
fn z_command_reaches_composing_buffer_without_space() {
    let mut engine = InputMethodEngine::new();
    type_string(&mut engine, "zl");
    assert_eq!(engine.input_buf.text, "→");
}
