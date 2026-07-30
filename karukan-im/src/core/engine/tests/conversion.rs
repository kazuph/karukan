use super::*;
use crate::config::settings::{Settings, StrategyMode};
use crate::core::engine::conversion_cache::ConversionResultKey;
use karukan_engine::{Dictionary, LearningCache};
use std::fs;
use std::io::Write;
use std::process::Command;

const P1_REQUEST_NUM_CANDIDATES: usize = 9;
const P1_GOLDEN_ORIGIN_MAIN: &str = "506978a72050fa928a4c90fb1fdddd1d10d0f5b9";
const P1_MAIN_MODEL_ID: &str = "jinen-v1-small-q5";
const P1_LIGHT_MODEL_ID: &str = "jinen-v1-xsmall-q5";
const P1_QUANTIZATION: &str = "Q5_K_M";
const P1_MAIN_MODEL_SHA256: &str =
    "9bbe15b5832291712b4d77ee7909c9ddcb9a6587891e932379480b4e99f13b52";
const P1_LIGHT_MODEL_SHA256: &str =
    "bb3110f06e539bf8596756df85a48b3946f1378e6cb912322b9c368be06d79aa";
const P1_INSTALLED_SYSTEM_DICT_SHA256: &str =
    "d85fed3c7e408e67f4a5dbe2314338366ed6183e0498d2726bc5a4dcaf5bb83b";
const P1_SYSTEM_DICT_INPUT: &str =
    r#"[{"reading":"ばーじょん","candidates":[{"surface":"P1_SYSTEM","score":1.0}]}]"#;
const P1_FIXTURE_SYSTEM_DICT_SHA256: &str =
    "29234ec6e1e3a1e21d52c358a45d3da75113512b2eddbde94e663b81125d9e41";
const P1_MAX_CONTEXT_LENGTH: usize = 10;
const P1_COMPOSING_CHUNK_LENGTH: usize = 30;
const P1_SHORT_INPUT_THRESHOLD: usize = 10;
const P1_BEAM_WIDTH: usize = 3;
const P1_MAX_LATENCY_MS: u64 = 100;
const P1_N_THREADS: u32 = 4;
const P1_LEARNING_MAX_ENTRIES: usize = 10_000;
const P1_LCTX: &str = "今日はいい";

fn candidate_list_snapshot(
    candidates: &CandidateList,
) -> (Vec<String>, Vec<String>, Option<usize>, Option<usize>) {
    (
        candidates
            .candidates()
            .iter()
            .map(|candidate| candidate.text.clone())
            .collect(),
        candidates
            .page_candidates()
            .iter()
            .map(|candidate| candidate.text.clone())
            .collect(),
        candidates.cursor(),
        candidates.page_cursor(),
    )
}

fn fixture_sha256(path: &std::path::Path) -> String {
    let output = Command::new("shasum")
        .args(["-a", "256"])
        .arg(path)
        .output()
        .expect("shasum must be available for the P1 fixture manifest");
    assert!(
        output.status.success(),
        "shasum must hash fixture artifacts"
    );
    String::from_utf8(output.stdout)
        .expect("shasum output must be UTF-8")
        .split_whitespace()
        .next()
        .expect("shasum output must include a digest")
        .to_string()
}

fn assert_p1_model_artifacts() {
    let main_path = karukan_engine::kanji::get_path_by_id(P1_MAIN_MODEL_ID)
        .expect("P1 main model artifact must resolve from the registry");
    let light_path = karukan_engine::kanji::get_path_by_id(P1_LIGHT_MODEL_ID)
        .expect("P1 light model artifact must resolve from the registry");
    assert_eq!(fixture_sha256(&main_path), P1_MAIN_MODEL_SHA256);
    assert_eq!(fixture_sha256(&light_path), P1_LIGHT_MODEL_SHA256);
}

fn assert_show_candidates_action_matches_state(result: &EngineResult, engine: &InputMethodEngine) {
    let action_candidates = result
        .actions
        .iter()
        .find_map(|action| match action {
            EngineAction::ShowCandidates(candidates) => Some(candidates),
            _ => None,
        })
        .expect("Space must emit ShowCandidates");
    let state_candidates = engine.candidates().expect("Space must enter conversion");

    assert_eq!(
        candidate_list_snapshot(action_candidates),
        candidate_list_snapshot(state_candidates),
        "ShowCandidates must carry the conversion-state candidates and page cursor"
    );
}

fn assert_p1_show_candidates_golden(
    result: &EngineResult,
    engine: &InputMethodEngine,
    expected_texts: &[&str],
) {
    assert_show_candidates_action_matches_state(result, engine);
    let expected = (
        expected_texts
            .iter()
            .map(|text| (*text).to_string())
            .collect::<Vec<_>>(),
        expected_texts
            .iter()
            .take(CandidateList::DEFAULT_PAGE_SIZE)
            .map(|text| (*text).to_string())
            .collect::<Vec<_>>(),
        Some(0),
        Some(0),
    );
    let action_candidates = result
        .actions
        .iter()
        .find_map(|action| match action {
            EngineAction::ShowCandidates(candidates) => Some(candidates),
            _ => None,
        })
        .expect("P1 conversion must show candidates");
    assert_eq!(candidate_list_snapshot(action_candidates), expected);
}

fn assert_p1_commit_golden(
    engine: &mut InputMethodEngine,
    expected_texts: &[&str],
    expected_commit: &str,
) -> (Vec<String>, Option<usize>, String) {
    let snapshot = p1_snapshot_and_commit(engine);
    assert_eq!(
        snapshot,
        (
            expected_texts
                .iter()
                .map(|text| (*text).to_string())
                .collect(),
            Some(0),
            expected_commit.to_string(),
        )
    );
    snapshot
}

fn p1_settings() -> Settings {
    let mut settings = Settings::default();
    settings.conversion.strategy = StrategyMode::Adaptive;
    settings.conversion.num_candidates = P1_REQUEST_NUM_CANDIDATES;
    settings.conversion.use_context = true;
    settings.conversion.max_context_length = P1_MAX_CONTEXT_LENGTH;
    settings.conversion.composing_chunk_len = P1_COMPOSING_CHUNK_LENGTH;
    settings.conversion.short_input_threshold = P1_SHORT_INPUT_THRESHOLD;
    settings.conversion.beam_width = P1_BEAM_WIDTH;
    settings.conversion.max_latency_ms = P1_MAX_LATENCY_MS;
    settings.conversion.model = Some(P1_MAIN_MODEL_ID.to_string());
    settings.conversion.light_model = Some(P1_LIGHT_MODEL_ID.to_string());
    settings.conversion.n_threads = P1_N_THREADS;
    settings.conversion.live_conversion = false;
    settings.conversion.tab_skips_learning = false;
    settings.learning.enabled = true;
    settings.learning.max_entries = P1_LEARNING_MAX_ENTRIES;
    settings
}

fn assert_p1_settings_manifest(settings: &Settings) {
    assert_eq!(settings.conversion.strategy, StrategyMode::Adaptive);
    assert_eq!(
        settings.conversion.num_candidates,
        P1_REQUEST_NUM_CANDIDATES
    );
    assert!(settings.conversion.use_context);
    assert_eq!(
        settings.conversion.max_context_length,
        P1_MAX_CONTEXT_LENGTH
    );
    assert_eq!(
        settings.conversion.composing_chunk_len,
        P1_COMPOSING_CHUNK_LENGTH
    );
    assert_eq!(
        settings.conversion.short_input_threshold,
        P1_SHORT_INPUT_THRESHOLD
    );
    assert_eq!(settings.conversion.beam_width, P1_BEAM_WIDTH);
    assert_eq!(settings.conversion.max_latency_ms, P1_MAX_LATENCY_MS);
    assert_eq!(settings.conversion.n_threads, P1_N_THREADS);
    assert!(!settings.conversion.live_conversion);
    assert!(!settings.conversion.tab_skips_learning);
    assert!(settings.learning.enabled);
    assert_eq!(settings.learning.max_entries, P1_LEARNING_MAX_ENTRIES);
}

fn build_fixture_dictionary(path: &std::path::Path, reading: &str, surface: &str) -> Dictionary {
    let input = if reading == "ばーじょん" && surface == "P1_SYSTEM" {
        P1_SYSTEM_DICT_INPUT.to_string()
    } else {
        format!(
            r#"[{{"reading":"{reading}","candidates":[{{"surface":"{surface}","score":1.0}}]}}]"#
        )
    };
    fs::write(path, input).expect("fixture dictionary input must be writable");
    Dictionary::build_from_json(path).expect("fixture dictionary input must build")
}

fn p1_production_merge_fixture() -> (Vec<String>, Option<usize>, String) {
    let expected_texts = [
        "P1_LEARNING",
        "P1_USER",
        "Karukan 0.1.0",
        "バージョン",
        "Version",
        "version",
        "P1_SYSTEM",
        "ばーじょん",
        "ﾊﾞｰｼﾞｮﾝ",
    ];
    let fixture_dir = tempfile::tempdir().expect("fixture directory must exist");
    let system_json = fixture_dir.path().join("p1-system.json");
    let system_bin = fixture_dir.path().join("p1-system.dict.bin");
    let system_dict = build_fixture_dictionary(&system_json, "ばーじょん", "P1_SYSTEM");
    system_dict
        .save(&system_bin)
        .expect("fixture system dictionary must save");
    assert_eq!(fixture_sha256(&system_bin), P1_FIXTURE_SYSTEM_DICT_SHA256);

    let user_dir = fixture_dir.path().join("user_dicts");
    fs::create_dir(&user_dir).expect("fixture user dictionary directory must exist");
    let mut user_file = fs::File::create(user_dir.join("p1-user.tsv"))
        .expect("fixture user dictionary must be writable");
    writeln!(user_file, "ばーじょん\tP1_USER\t名詞\tfixture")
        .expect("fixture user dictionary must be complete");

    let mut settings = p1_settings();
    assert_p1_settings_manifest(&settings);
    settings.conversion.dict_path = Some(system_bin.to_string_lossy().into_owned());
    let mut engine = InputMethodEngine::with_config(EngineConfig::from_settings(&settings));
    engine
        .init_from_settings(&settings)
        .expect("P1 production settings must initialize real models");
    engine.init_user_dictionaries_with_dir(&user_dir);

    let mut learning = LearningCache::new(settings.learning.max_entries);
    learning.record("ばーじょん", "P1_LEARNING");
    engine.learning = Some(learning);
    engine.input_buf.text = "ばーじょん".to_string();
    engine.input_buf.cursor_pos = "ばーじょん".chars().count();
    engine.state = InputState::Composing {
        preedit: Preedit::new(),
        romaji_buffer: String::new(),
    };

    assert_eq!(engine.config.num_candidates, P1_REQUEST_NUM_CANDIDATES);
    assert_eq!(
        engine.determine_strategy("ばーじょん", P1_REQUEST_NUM_CANDIDATES),
        ConversionStrategy::ParallelBeam {
            beam_width: P1_BEAM_WIDTH
        }
    );
    assert!(!engine.metrics.adaptive_use_light_model);
    let space = engine.process_key(&press_key(Keysym::SPACE));
    assert!(
        engine.metrics.conversion_ms > 0,
        "production fixture Space must perform model inference, not a cache hit"
    );
    assert_p1_show_candidates_golden(&space, &engine, &expected_texts);

    let candidates = engine.candidates().expect("Space must enter conversion");
    let texts = candidates
        .candidates()
        .iter()
        .map(|candidate| candidate.text.clone())
        .collect::<Vec<_>>();
    let labels = candidates
        .candidates()
        .iter()
        .map(|candidate| candidate.source_label.as_deref())
        .collect::<Vec<_>>();
    assert_eq!(
        texts,
        expected_texts
            .iter()
            .map(|text| (*text).to_string())
            .collect::<Vec<_>>()
    );
    assert_eq!(texts.first().map(String::as_str), Some("P1_LEARNING"));
    assert_eq!(labels.first(), Some(&Some("📝 学習")));
    assert_eq!(texts.get(1).map(String::as_str), Some("P1_USER"));
    assert_eq!(labels.get(1), Some(&Some("👤 ユーザー")));

    let special_index = texts
        .iter()
        .position(|text| text == "Karukan 0.1.0")
        .expect("fixture must include the deterministic special conversion");
    let model_index = labels
        .iter()
        .position(|label| *label == Some("🤖 AI"))
        .expect("fixture must include real model output");
    let system_index = texts
        .iter()
        .position(|text| text == "P1_SYSTEM")
        .expect("fixture must include the real system dictionary");
    let fallback_index = texts
        .iter()
        .position(|text| text == "ばーじょん")
        .expect("fixture must include the hiragana fallback");
    assert!(special_index < model_index);
    assert!(model_index < system_index);
    assert!(system_index < fallback_index);
    let cursor = candidates.cursor();
    assert_eq!(cursor, Some(0));

    let snapshot = assert_p1_commit_golden(&mut engine, &expected_texts, "P1_LEARNING");
    assert_eq!(snapshot.0, texts);
    assert_eq!(snapshot.1, cursor);
    snapshot
}

fn p1_thread_budgets() -> [(Option<ParallelBeamThreadBudget>, &'static str); 4] {
    [
        (None, "4/4 default"),
        (ParallelBeamThreadBudget::new(2, 2), "2/2"),
        (ParallelBeamThreadBudget::new(2, 1), "2/1"),
        (ParallelBeamThreadBudget::new(1, 1), "1/1"),
    ]
}

fn p1_engine(parallel_beam_thread_budget: Option<ParallelBeamThreadBudget>) -> InputMethodEngine {
    let settings = p1_settings();
    assert_p1_settings_manifest(&settings);
    let mut engine = InputMethodEngine::with_config(EngineConfig::from_settings(&settings));
    engine
        .init_kanji_converter_with_model(P1_MAIN_MODEL_ID, P1_N_THREADS)
        .expect("P1 main model must load");
    engine
        .init_light_kanji_converter(P1_LIGHT_MODEL_ID, P1_N_THREADS)
        .expect("P1 light model must load");
    engine.set_parallel_beam_thread_budget(parallel_beam_thread_budget);
    assert_p1_model_artifacts();
    engine
}

fn p1_initial_display_equivalence_fixture(
    engine: &mut InputMethodEngine,
) -> (Vec<String>, Option<usize>, String) {
    let expected_texts = [
        "天気",
        "転機",
        "転記",
        "てんき",
        "テンキ",
        "ﾃﾝｷ",
        "☀",
        "☼",
        "☁",
        "☂",
        "☔",
        "☃",
        "☀\u{fe0f}",
        "☁\u{fe0f}",
        "☂\u{fe0f}",
        "☔\u{fe0f}",
        "⛅\u{fe0f}",
        "⛈\u{fe0f}",
        "❄\u{fe0f}",
        "🌀",
        "🌂",
        "🌈",
        "🌊",
        "🌡\u{fe0f}",
        "🌤\u{fe0f}",
        "🌥\u{fe0f}",
        "🌦\u{fe0f}",
        "🌧\u{fe0f}",
        "🌨\u{fe0f}",
        "🌩\u{fe0f}",
        "🌪\u{fe0f}",
        "🌫\u{fe0f}",
    ];
    engine.reset();
    engine.conversion_result_cache = Default::default();
    engine.set_surrounding_context(P1_LCTX, "");
    assert_eq!(engine.truncate_context_for_api(), P1_LCTX);
    assert_eq!(
        engine.config.num_candidates, P1_REQUEST_NUM_CANDIDATES,
        "P1 Space must request nine candidates; deduplication may display fewer"
    );

    for ch in "tenki".chars() {
        engine.process_key(&press(ch));
    }
    assert_eq!(
        engine.determine_strategy("てんき", P1_REQUEST_NUM_CANDIDATES),
        ConversionStrategy::ParallelBeam {
            beam_width: P1_BEAM_WIDTH
        }
    );
    let space = engine.process_key(&press_key(Keysym::SPACE));
    assert!(
        engine.metrics.conversion_ms > 0,
        "each fixture run must perform model inference, not a conversion-cache hit"
    );
    assert_p1_show_candidates_golden(&space, engine, &expected_texts);

    let candidates = engine.candidates().expect("Space must enter conversion");
    assert_eq!(candidates.cursor(), Some(0), "P1 selects the first row");
    let first = candidates
        .candidates()
        .first()
        .expect("real model must produce a first candidate");
    assert_eq!(
        first.source_label.as_deref(),
        Some("🤖 AI"),
        "P1 first candidate must be model output, not a fallback"
    );
    assert!(
        first
            .text
            .chars()
            .any(|ch| ('\u{4E00}'..='\u{9FFF}').contains(&ch)),
        "P1 first model candidate must contain kanji"
    );
    let first_text = first.text.clone();
    assert_eq!(first_text, expected_texts[0]);
    let snapshot = assert_p1_commit_golden(engine, &expected_texts, expected_texts[0]);
    assert_eq!(snapshot.2, first_text);
    snapshot
}

fn p1_prepare_reading(engine: &mut InputMethodEngine, reading: &str) -> EngineResult {
    engine.reset();
    engine.conversion_result_cache = Default::default();
    engine.set_surrounding_context(P1_LCTX, "");
    engine.input_buf.text = reading.to_string();
    engine.input_buf.cursor_pos = reading.chars().count();
    engine.state = InputState::Composing {
        preedit: Preedit::new(),
        romaji_buffer: String::new(),
    };
    assert!(!engine.metrics.adaptive_use_light_model);

    let space = engine.process_key(&press_key(Keysym::SPACE));
    assert!(
        engine.metrics.conversion_ms > 0,
        "P1 `{reading}` must run model inference rather than use the conversion cache"
    );
    assert_show_candidates_action_matches_state(&space, engine);
    space
}

fn p1_snapshot_and_commit(engine: &mut InputMethodEngine) -> (Vec<String>, Option<usize>, String) {
    let candidates = engine.candidates().expect("P1 state must be Conversion");
    let texts = candidates
        .candidates()
        .iter()
        .map(|candidate| candidate.text.clone())
        .collect::<Vec<_>>();
    let cursor = candidates.cursor();
    let selected = candidates
        .selected_text()
        .expect("P1 candidate list must select the first candidate")
        .to_string();
    let history = engine
        .conversion_history
        .iter()
        .map(|segment| segment.surface.as_str())
        .collect::<String>();
    let remainder = engine
        .input_buf
        .text
        .chars()
        .skip(engine.input_buf.cursor_pos)
        .collect::<String>();
    let expected_commit = format!("{history}{selected}{remainder}");
    let commit = engine.process_key(&press_key(Keysym::RETURN));
    let committed = commit.actions.iter().find_map(|action| match action {
        EngineAction::Commit(text) => Some(text.clone()),
        _ => None,
    });
    assert_eq!(committed.as_deref(), Some(expected_commit.as_str()));
    (texts, cursor, expected_commit)
}

fn p1_target_reading(engine: &InputMethodEngine) -> String {
    engine
        .input_buf
        .text
        .chars()
        .take(engine.input_buf.cursor_pos)
        .collect()
}

fn assert_p1_strategy_for_flag(engine: &InputMethodEngine, adaptive: bool) {
    let expected = if adaptive {
        ConversionStrategy::LightModelOnly
    } else {
        ConversionStrategy::ParallelBeam {
            beam_width: P1_BEAM_WIDTH,
        }
    };
    assert_eq!(
        engine.determine_strategy(&p1_target_reading(engine), P1_REQUEST_NUM_CANDIDATES),
        expected
    );
}

#[test]
fn p1_initial_display_equivalence_fixture_is_repeatable() {
    eprintln!(
        "P1 golden manifest: origin={P1_GOLDEN_ORIGIN_MAIN} main={P1_MAIN_MODEL_ID}/{P1_QUANTIZATION}@{P1_MAIN_MODEL_SHA256} light={P1_LIGHT_MODEL_ID}/{P1_QUANTIZATION}@{P1_LIGHT_MODEL_SHA256} installed_system_dict_sha256={P1_INSTALLED_SYSTEM_DICT_SHA256} fixture_system_dict_input={P1_SYSTEM_DICT_INPUT} fixture_system_dict_sha256={P1_FIXTURE_SYSTEM_DICT_SHA256} strategy=adaptive initial_space_strategy=parallel_beam num_candidates={P1_REQUEST_NUM_CANDIDATES} use_context=true max_context_length={P1_MAX_CONTEXT_LENGTH} composing_chunk_len={P1_COMPOSING_CHUNK_LENGTH} beam_width={P1_BEAM_WIDTH} short_input_threshold={P1_SHORT_INPUT_THRESHOLD} max_latency_ms={P1_MAX_LATENCY_MS} n_threads={P1_N_THREADS} live_conversion=false tab_skips_learning=false learning_enabled=true learning_max_entries={P1_LEARNING_MAX_ENTRIES} initial_adaptive=false lctx={P1_LCTX:?}"
    );
    for (budget, budget_label) in p1_thread_budgets() {
        eprintln!("P1 initial fixture budget={budget_label}");
        let mut engine = p1_engine(budget);
        let first = p1_initial_display_equivalence_fixture(&mut engine);
        eprintln!("P1 equivalence fixture budget={budget_label}: {first:?}");
        for run in 2..=5 {
            let repeated = p1_initial_display_equivalence_fixture(&mut engine);
            assert_eq!(
                repeated, first,
                "ParallelBeam budget={budget_label} output must be repeatable on run {run}"
            );
        }
    }
}

#[test]
fn p1_production_merge_fixture_uses_settings_and_real_sources() {
    let result = p1_production_merge_fixture();
    eprintln!("P1 production merge fixture: {result:?}");
}

#[test]
fn p1_release_candidate_matrix_matches_golden_without_conversion_cache() {
    let cases: Vec<(&str, Vec<String>, &str)> = vec![
        (
            "まにあってない",
            vec![
                "間に合ってない".to_string(),
                "間にあってない".to_string(),
                "間に合って無い".to_string(),
                "まにあってない".to_string(),
                "マニアッテナイ".to_string(),
                "ﾏﾆｱｯﾃﾅｲ".to_string(),
            ],
            "間に合ってない",
        ),
        (
            "なにがげんいんかわかる",
            vec![
                "なにが原因かわかる".to_string(),
                "なにが原因か分かる".to_string(),
                "何が原因かわかる".to_string(),
                "なにがげんいんかわかる".to_string(),
                "ナニガゲンインカワカル".to_string(),
                "ﾅﾆｶﾞｹﾞﾝｲﾝｶﾜｶﾙ".to_string(),
            ],
            "なにが原因かわかる",
        ),
        (
            "てんき",
            vec![
                "天気".to_string(),
                "転機".to_string(),
                "転記".to_string(),
                "てんき".to_string(),
                "テンキ".to_string(),
                "ﾃﾝｷ".to_string(),
                "☀".to_string(),
                "☼".to_string(),
                "☁".to_string(),
                "☂".to_string(),
                "☔".to_string(),
                "☃".to_string(),
                "☀\u{fe0f}".to_string(),
                "☁\u{fe0f}".to_string(),
                "☂\u{fe0f}".to_string(),
                "☔\u{fe0f}".to_string(),
                "⛅\u{fe0f}".to_string(),
                "⛈\u{fe0f}".to_string(),
                "❄\u{fe0f}".to_string(),
                "🌀".to_string(),
                "🌂".to_string(),
                "🌈".to_string(),
                "🌊".to_string(),
                "🌡\u{fe0f}".to_string(),
                "🌤\u{fe0f}".to_string(),
                "🌥\u{fe0f}".to_string(),
                "🌦\u{fe0f}".to_string(),
                "🌧\u{fe0f}".to_string(),
                "🌨\u{fe0f}".to_string(),
                "🌩\u{fe0f}".to_string(),
                "🌪\u{fe0f}".to_string(),
                "🌫\u{fe0f}".to_string(),
            ],
            "天気",
        ),
        (
            "てすとa1!",
            vec![
                "テストa1!".to_string(),
                "testa1!".to_string(),
                "てすとa1!".to_string(),
            ],
            "テストa1!",
        ),
        (
            "なにがげんいんかわかるなにがげんいんかわかるなにがげんいんかわかるなにがげんいんかわかる",
            vec![
                "なにが原因かわかるなにが原因かわかるなにが原因かわかるなにが原因かわかる"
                    .to_string(),
                "なにがげんいんかわかるなにがげんいんかわかるなにがげんいんかわかるなにがげんいんかわかる"
                    .to_string(),
                "ナニガゲンインカワカルナニガゲンインカワカルナニガゲンインカワカルナニガゲンインカワカル"
                    .to_string(),
                "ﾅﾆｶﾞｹﾞﾝｲﾝｶﾜｶﾙﾅﾆｶﾞｹﾞﾝｲﾝｶﾜｶﾙﾅﾆｶﾞｹﾞﾝｲﾝｶﾜｶﾙﾅﾆｶﾞｹﾞﾝｲﾝｶﾜｶﾙ"
                    .to_string(),
            ],
            "なにが原因かわかるなにが原因かわかるなにが原因かわかるなにが原因かわかる",
        ),
    ];
    for (budget, budget_label) in p1_thread_budgets() {
        eprintln!("P1 matrix fixture budget={budget_label}");
        let mut engine = p1_engine(budget);
        for (reading, expected_texts, expected_commit) in &cases {
            for run in 1..=5 {
                let space = p1_prepare_reading(&mut engine, reading);
                let expected_refs = expected_texts
                    .iter()
                    .map(String::as_str)
                    .collect::<Vec<_>>();
                assert_p1_show_candidates_golden(&space, &engine, &expected_refs);
                let snapshot =
                    assert_p1_commit_golden(&mut engine, &expected_refs, expected_commit);
                assert_eq!(
                    snapshot.1,
                    Some(0),
                    "P1 budget={budget_label} `{reading}` run {run}"
                );
            }
        }
    }
}

#[test]
fn p1_plain_arrows_preserve_non_inference_golden() {
    let reading = "なにがげんいんかわかる";
    let expected_texts = [
        "なにが原因かわかる",
        "なにが原因か分かる",
        "何が原因かわかる",
        "なにがげんいんかわかる",
        "ナニガゲンインカワカル",
        "ﾅﾆｶﾞｹﾞﾝｲﾝｶﾜｶﾙ",
    ];
    for (budget, budget_label) in p1_thread_budgets() {
        eprintln!("P1 plain-arrow fixture budget={budget_label}");
        let mut engine = p1_engine(budget);
        for run in 1..=5 {
            let space = p1_prepare_reading(&mut engine, reading);
            assert_p1_show_candidates_golden(&space, &engine, &expected_texts);
            let left = engine.process_key(&press_key(Keysym::LEFT));
            let right = engine.process_key(&press_key(Keysym::RIGHT));
            assert!(
                left.consumed && right.consumed,
                "plain arrows budget={budget_label} run {run}"
            );
            assert!(left.actions.is_empty() && right.actions.is_empty());
            assert_eq!(
                engine.metrics.conversion_ms, 0,
                "plain arrows budget={budget_label} must not run inference on run {run}"
            );
            assert_eq!(
                candidate_list_snapshot(engine.candidates().expect("P1 candidates remain visible")),
                (
                    expected_texts
                        .iter()
                        .map(|text| (*text).to_string())
                        .collect(),
                    expected_texts
                        .iter()
                        .map(|text| (*text).to_string())
                        .collect(),
                    Some(0),
                    Some(0),
                ),
                "plain arrows budget={budget_label} must preserve P1 candidates"
            );
            assert_p1_commit_golden(&mut engine, &expected_texts, "なにが原因かわかる");
        }
    }
}

#[test]
fn p1_shift_and_escape_goldens_are_strategy_specific() {
    let reading = "なにがげんいんかわかる";

    for (budget, budget_label) in p1_thread_budgets() {
        eprintln!("P1 Shift/Esc fixture budget={budget_label}");
        let mut engine = p1_engine(budget);
        for adaptive in [false, true] {
            let expected_left = if adaptive {
                (
                    vec![
                        "なにが原因乾か",
                        "なにがげんいんかわか",
                        "ナニガゲンインカワカ",
                        "ﾅﾆｶﾞｹﾞﾝｲﾝｶﾜｶ",
                    ],
                    "なにが原因乾かる",
                )
            } else {
                (
                    vec![
                        "なにが原因かわか",
                        "なにが原因乾か",
                        "なにが原因か若",
                        "なにがげんいんかわか",
                        "ナニガゲンインカワカ",
                        "ﾅﾆｶﾞｹﾞﾝｲﾝｶﾜｶ",
                    ],
                    "なにが原因かわかる",
                )
            };
            let expected_full = if adaptive {
                (
                    vec![
                        "なにが原因かわかる",
                        "なにがげんいんかわかる",
                        "ナニガゲンインカワカル",
                        "ﾅﾆｶﾞｹﾞﾝｲﾝｶﾜｶﾙ",
                    ],
                    "なにが原因かわかる",
                )
            } else {
                (
                    vec![
                        "なにが原因かわかる",
                        "なにが原因か分かる",
                        "何が原因かわかる",
                        "なにがげんいんかわかる",
                        "ナニガゲンインカワカル",
                        "ﾅﾆｶﾞｹﾞﾝｲﾝｶﾜｶﾙ",
                    ],
                    "なにが原因かわかる",
                )
            };

            for run in 1..=5 {
                p1_prepare_reading(&mut engine, reading);
                engine.metrics.adaptive_use_light_model = adaptive;
                engine.conversion_result_cache = Default::default();
                let shifted_left_reading = reading
                    .chars()
                    .take(reading.chars().count() - 1)
                    .collect::<String>();
                let expected = if adaptive {
                    ConversionStrategy::LightModelOnly
                } else {
                    ConversionStrategy::ParallelBeam {
                        beam_width: P1_BEAM_WIDTH,
                    }
                };
                assert_eq!(
                    engine.determine_strategy(&shifted_left_reading, P1_REQUEST_NUM_CANDIDATES),
                    expected
                );
                let left = engine.process_key(&press_shift_key(Keysym::LEFT));
                assert!(engine.metrics.conversion_ms > 0);
                assert_p1_show_candidates_golden(&left, &engine, &expected_left.0);
                let left_snapshot = p1_snapshot_and_commit(&mut engine);

                p1_prepare_reading(&mut engine, reading);
                engine.metrics.adaptive_use_light_model = adaptive;
                engine.conversion_result_cache = Default::default();
                engine.process_key(&press_shift_key(Keysym::LEFT));
                engine.metrics.adaptive_use_light_model = adaptive;
                engine.conversion_result_cache = Default::default();
                assert_eq!(
                    engine.determine_strategy(reading, P1_REQUEST_NUM_CANDIDATES),
                    expected
                );
                let right = engine.process_key(&press_shift_key(Keysym::RIGHT));
                assert!(engine.metrics.conversion_ms > 0);
                assert_p1_show_candidates_golden(&right, &engine, &expected_full.0);
                let right_snapshot = p1_snapshot_and_commit(&mut engine);

                p1_prepare_reading(&mut engine, reading);
                let escaped = engine.process_key(&press_key(Keysym::ESCAPE));
                assert!(escaped.consumed);
                engine.metrics.adaptive_use_light_model = adaptive;
                engine.conversion_result_cache = Default::default();
                assert_p1_strategy_for_flag(&engine, adaptive);
                let re_spaced = engine.process_key(&press_key(Keysym::SPACE));
                assert!(engine.metrics.conversion_ms > 0);
                assert_p1_show_candidates_golden(&re_spaced, &engine, &expected_full.0);
                let escape_snapshot = p1_snapshot_and_commit(&mut engine);

                let golden = |expected: &(Vec<&str>, &str)| {
                    (
                        expected
                            .0
                            .iter()
                            .map(|text| (*text).to_string())
                            .collect::<Vec<_>>(),
                        Some(0),
                        expected.1.to_string(),
                    )
                };
                assert_eq!(
                    left_snapshot,
                    golden(&expected_left),
                    "Shift+Left budget={budget_label} adaptive={adaptive} run {run}"
                );
                assert_eq!(
                    right_snapshot,
                    golden(&expected_full),
                    "Shift+Right budget={budget_label} adaptive={adaptive} run {run}"
                );
                assert_eq!(
                    escape_snapshot,
                    golden(&expected_full),
                    "Esc→Space budget={budget_label} adaptive={adaptive} run {run}"
                );
            }
        }
    }
}

#[test]
fn p1_adaptive_transition_preserves_flag_across_escape() {
    for (budget, budget_label) in p1_thread_budgets() {
        eprintln!("P1 adaptive-transition fixture budget={budget_label}");
        let mut engine = p1_engine(budget);
        p1_prepare_reading(&mut engine, "なにがげんいんかわかる");
        let adaptive = engine.metrics.adaptive_use_light_model;
        assert_eq!(
            adaptive,
            engine.metrics.conversion_ms > P1_MAX_LATENCY_MS,
            "adaptive transition budget={budget_label}"
        );
        let escaped = engine.process_key(&press_key(Keysym::ESCAPE));
        assert!(escaped.consumed, "Esc budget={budget_label}");
        assert!(
            matches!(engine.state(), InputState::Composing { .. }),
            "Esc budget={budget_label} must return to Composing"
        );
        assert_eq!(
            engine.metrics.adaptive_use_light_model, adaptive,
            "Esc budget={budget_label} must preserve adaptive flag"
        );
        assert_p1_strategy_for_flag(&engine, adaptive);
    }
}

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
