//! AJIMEE-Bench evaluation through the engine's Space-conversion path.
//!
//! Run with:
//!   KARUKAN_SPACE_EVAL_ITEMS=<evaluation_items.json> \
//!   KARUKAN_SPACE_EVAL_OUT=<out.json> \
//!   cargo test --release -p karukan-im space_eval_ajimee -- --ignored --nocapture
//!
//! The engine is initialized with the production adaptive settings and real
//! models but no system/user dictionary and no learning cache, so the model
//! candidates are measured in isolation. `context_text` is passed as the
//! surrounding (left) context, exactly as the JSON-RPC frontend delivers it.

use super::*;
use crate::config::settings::Settings;
use serde::Deserialize;
use serde_json::json;

#[derive(Debug, Deserialize)]
struct EvalItem {
    input: String,
    context_text: Option<String>,
    expected_output: Vec<String>,
}

fn eval_engine(settings: &Settings) -> InputMethodEngine {
    let mut engine = InputMethodEngine::with_config(EngineConfig::from_settings(settings));
    let main = settings
        .conversion
        .model
        .clone()
        .unwrap_or_else(|| karukan_engine::kanji::registry().default_model.clone());
    engine
        .init_kanji_converter_with_model(&main, settings.conversion.n_threads)
        .expect("eval main model must load");
    let light = settings
        .conversion
        .light_model
        .clone()
        .unwrap_or_else(|| karukan_engine::kanji::registry().default_model.clone());
    engine
        .init_light_kanji_converter(&light, settings.conversion.n_threads)
        .expect("eval light model must load");
    engine
}

#[test]
#[ignore = "AJIMEE-Bench measurement harness; run explicitly"]
fn space_eval_ajimee() {
    let items_path = std::env::var("KARUKAN_SPACE_EVAL_ITEMS")
        .expect("KARUKAN_SPACE_EVAL_ITEMS must point at evaluation_items.json");
    let out_path =
        std::env::var("KARUKAN_SPACE_EVAL_OUT").expect("KARUKAN_SPACE_EVAL_OUT must be set");

    let data =
        std::fs::read_to_string(&items_path).expect("evaluation_items.json must be readable");
    let items: Vec<EvalItem> = serde_json::from_str(&data).expect("evaluation items must parse");

    let mut settings = Settings::default();
    settings.conversion.prefetch_delay_ms = 0;
    let mut engine = eval_engine(&settings);

    let mut records = Vec::with_capacity(items.len());
    for (idx, item) in items.iter().enumerate() {
        engine.reset();
        engine.conversion_result_cache = Default::default();
        if let Some(ctx) = &item.context_text {
            engine.set_surrounding_context(ctx, "");
        }
        engine.input_buf.text = item.input.clone();
        engine.input_buf.cursor_pos = item.input.chars().count();
        engine.state = InputState::Composing {
            preedit: Preedit::new(),
            romaji_buffer: String::new(),
        };
        engine.metrics.adaptive_use_light_model = false;

        let started = std::time::Instant::now();
        engine.process_key(&press_key(Keysym::SPACE));
        let wall_ms = started.elapsed().as_secs_f64() * 1000.0;

        let candidates: Vec<String> = engine
            .candidates()
            .map(|list| {
                list.candidates()
                    .iter()
                    .map(|candidate| candidate.text.clone())
                    .collect()
            })
            .unwrap_or_default();
        let model_candidates: Vec<String> = engine
            .candidates()
            .map(|list| {
                list.candidates()
                    .iter()
                    .filter(|candidate| candidate.source_label.as_deref() == Some("🤖 AI"))
                    .map(|candidate| candidate.text.clone())
                    .collect()
            })
            .unwrap_or_default();

        records.push(json!({
            "index": idx,
            "input": item.input,
            "context_text": item.context_text,
            "expected_output": item.expected_output,
            "candidates": candidates,
            "model_candidates": model_candidates,
            "model_name": engine.metrics.model_name,
            "conversion_ms": engine.metrics.conversion_ms,
            "wall_ms": wall_ms,
        }));
        eprintln!(
            "[{}/{}] {} -> {:?}",
            idx + 1,
            items.len(),
            item.input,
            candidates.first()
        );
    }

    let output = json!({
        "model_name": engine.model_name(),
        "settings": {
            "strategy": format!("{:?}", settings.conversion.strategy),
            "num_candidates": settings.conversion.num_candidates,
            "beam_width": settings.conversion.beam_width,
            "n_threads": settings.conversion.n_threads,
            "use_context": settings.conversion.use_context,
            "max_context_length": settings.conversion.max_context_length,
        },
        "items": records,
    });
    std::fs::write(&out_path, serde_json::to_string_pretty(&output).unwrap())
        .expect("eval output must be writable");
    eprintln!("wrote {out_path}");
}

#[test]
fn space_main_beam_contains_requested_corrections() {
    let mut settings = Settings::default();
    settings.conversion.prefetch_delay_ms = 0;
    let mut engine = eval_engine(&settings);
    for (romaji, expected) in [
        ("konokizunaosenaino", "この傷治せないの"),
        ("yorugohanwotabeniitta", "夜ご飯を食べに行った"),
    ] {
        engine.reset();
        for ch in romaji.chars() {
            engine.process_key(&press(ch));
        }
        engine.metrics.adaptive_use_light_model = true;
        engine.process_key(&press_key(Keysym::SPACE));
        let candidates = engine.candidates().unwrap().candidates();
        assert!(
            candidates.iter().take(3).any(|c| c.text == expected),
            "{romaji}: {candidates:?}"
        );
        assert_eq!(engine.metrics.model_name, "jinen-v2-small-q5");
    }
    // Even a one-row Space list must never use adaptive's live fallback.
    engine.reset();
    engine.config.num_candidates = 1;
    for ch in "yorugohanwotabeniitta".chars() {
        engine.process_key(&press(ch));
    }
    engine.metrics.adaptive_use_light_model = true;
    engine.process_key(&press_key(Keysym::SPACE));
    assert_eq!(engine.metrics.model_name, "jinen-v2-small-q5");
}

#[test]
fn space_prefetch_matches_inline_and_discards_edited_reading() {
    let mut settings = Settings::default();
    settings.conversion.prefetch_delay_ms = 0;
    let mut inline = eval_engine(&settings);
    settings.conversion.prefetch_delay_ms = 1;
    let mut prefetch = eval_engine(&settings);
    for engine in [&mut inline, &mut prefetch] {
        for ch in "konokizunaosenai".chars() {
            engine.process_key(&press(ch));
        }
        // Exercise a completed or in-flight result followed by a new key.
        std::thread::sleep(std::time::Duration::from_millis(10));
        for ch in "no".chars() {
            engine.process_key(&press(ch));
        }
        engine.process_key(&press_key(Keysym::SPACE));
    }
    let texts = |engine: &InputMethodEngine| {
        engine
            .candidates()
            .unwrap()
            .candidates()
            .iter()
            .map(|c| c.text.clone())
            .collect::<Vec<_>>()
    };
    assert_eq!(texts(&inline), texts(&prefetch));
    assert_eq!(prefetch.metrics.model_name, "jinen-v2-small-q5");
    prefetch.reset();
    for ch in "yorugohanwotabeniitta".chars() {
        prefetch.process_key(&press(ch));
    }
    prefetch
        .space_prefetcher
        .as_ref()
        .unwrap()
        .wait_until_idle();
    let key = super::super::conversion_cache::ConversionResultKey {
        reading: "よるごはんをたべにいった".into(),
        left_context: String::new(),
        num_candidates: settings.conversion.num_candidates,
    };
    assert!(
        prefetch
            .conversion_result_cache
            .get(
                &key,
                &ConversionStrategy::MainModelBeam {
                    beam_width: settings.conversion.beam_width
                }
            )
            .is_some()
    );
    prefetch.process_key(&press_key(Keysym::SPACE));
    assert_eq!(prefetch.metrics.conversion_ms, 0);
    assert!(
        texts(&prefetch)
            .iter()
            .take(3)
            .any(|c| c == "夜ご飯を食べに行った")
    );
}
