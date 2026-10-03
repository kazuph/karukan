use super::super::strategy::determine_conversion_strategy;
use super::*;

// --- ConversionStrategy tests ---

/// Helper to create a config with specific thresholds
fn strategy_config(short_input_threshold: usize, beam_width: usize) -> EngineConfig {
    EngineConfig {
        short_input_threshold,
        beam_width,
        num_candidates: 9,
        ..EngineConfig::default()
    }
}

/// Default test config: short_input_threshold=10, beam_width=3, max_latency_ms=100
fn default_strategy_config() -> EngineConfig {
    strategy_config(10, 3)
}

#[test]
fn strategy_no_light_model_returns_main_model_only() {
    let config = default_strategy_config();
    assert_eq!(
        determine_conversion_strategy(5, 1, false, false, &config),
        ConversionStrategy::MainModelOnly,
    );
    assert_eq!(
        determine_conversion_strategy(5, 9, false, false, &config),
        ConversionStrategy::MainModelBeam { beam_width: 3 },
    );
    assert_eq!(
        determine_conversion_strategy(50, 9, false, true, &config),
        ConversionStrategy::MainModelBeam { beam_width: 3 },
    );
}

// --- Auto-suggest (num_candidates == 1) ---

#[test]
fn strategy_auto_suggest_adaptive_false_returns_main_model() {
    let config = default_strategy_config();
    assert_eq!(
        determine_conversion_strategy(5, 1, true, false, &config),
        ConversionStrategy::MainModelOnly,
    );
}

#[test]
fn strategy_auto_suggest_adaptive_true_returns_light_model() {
    let config = default_strategy_config();
    assert_eq!(
        determine_conversion_strategy(5, 1, true, true, &config),
        ConversionStrategy::LightModelOnly,
    );
}

#[test]
fn strategy_auto_suggest_adaptive_true_even_short_input() {
    let config = default_strategy_config();
    assert_eq!(
        determine_conversion_strategy(1, 1, true, true, &config),
        ConversionStrategy::LightModelOnly,
    );
}

// --- Explicit conversion (num_candidates > 1) ---

#[test]
fn strategy_explicit_adaptive_true_returns_main_beam() {
    let config = default_strategy_config();
    assert_eq!(
        determine_conversion_strategy(5, 9, true, true, &config),
        ConversionStrategy::MainModelBeam { beam_width: 3 },
    );
}

#[test]
fn strategy_explicit_short_reading_returns_main_beam() {
    let config = default_strategy_config();
    assert_eq!(
        determine_conversion_strategy(5, 9, true, false, &config),
        ConversionStrategy::MainModelBeam { beam_width: 3 },
    );
}

#[test]
fn strategy_explicit_long_reading_returns_main_beam() {
    let config = default_strategy_config();
    assert_eq!(
        determine_conversion_strategy(15, 9, true, false, &config),
        ConversionStrategy::MainModelBeam { beam_width: 3 },
    );
}

#[test]
fn strategy_explicit_reading_boundary_at_threshold() {
    let config = default_strategy_config();
    assert_eq!(
        determine_conversion_strategy(10, 9, true, false, &config),
        ConversionStrategy::MainModelBeam { beam_width: 3 },
    );
    assert_eq!(
        determine_conversion_strategy(11, 9, true, false, &config),
        ConversionStrategy::MainModelBeam { beam_width: 3 },
    );
}

// --- beam_width capping ---

#[test]
fn strategy_beam_width_capped_by_num_candidates() {
    let config = strategy_config(10, 5);
    assert_eq!(
        determine_conversion_strategy(5, 2, true, false, &config),
        ConversionStrategy::MainModelBeam { beam_width: 2 },
    );
}

#[test]
fn strategy_beam_width_capped_by_beam_width() {
    let config = strategy_config(10, 3);
    assert_eq!(
        determine_conversion_strategy(5, 9, true, false, &config),
        ConversionStrategy::MainModelBeam { beam_width: 3 },
    );
}

// --- Adaptive latency-based model switching ---

#[test]
fn strategy_adaptive_flag_does_not_override_space_beam() {
    let config = default_strategy_config();
    assert_eq!(
        determine_conversion_strategy(3, 9, true, true, &config),
        ConversionStrategy::MainModelBeam { beam_width: 3 },
    );
}

#[test]
fn strategy_adaptive_false_long_reading_still_uses_main_beam() {
    let config = default_strategy_config();
    assert_eq!(
        determine_conversion_strategy(20, 9, true, false, &config),
        ConversionStrategy::MainModelBeam { beam_width: 3 },
    );
}

// --- Engine-level adaptive flag behavior ---

#[test]
fn test_adaptive_flag_initial_state() {
    let engine = InputMethodEngine::new();
    assert!(!engine.metrics.adaptive_use_light_model);
}

#[test]
fn test_adaptive_flag_reset_on_engine_reset() {
    let mut engine = InputMethodEngine::new();
    engine.metrics.adaptive_use_light_model = true;
    engine.reset();
    assert!(!engine.metrics.adaptive_use_light_model);
}

#[test]
fn test_adaptive_flag_reset_on_new_word() {
    let mut engine = InputMethodEngine::new();
    engine.metrics.adaptive_use_light_model = true;

    engine.process_key(&press('a'));
    assert!(!engine.metrics.adaptive_use_light_model);
}

#[test]
fn test_adaptive_flag_persists_during_input() {
    let mut engine = InputMethodEngine::new();

    // Start typing (flag reset on first key from Empty)
    engine.process_key(&press('a'));
    assert!(!engine.metrics.adaptive_use_light_model);

    // Manually set the flag (simulating slow conversion)
    engine.metrics.adaptive_use_light_model = true;

    // Continue typing — flag should persist (not in Empty state)
    engine.process_key(&press('i'));
    assert!(engine.metrics.adaptive_use_light_model);
}

#[test]
fn test_adaptive_flag_reset_after_commit_and_new_input() {
    let mut engine = InputMethodEngine::new();

    // Type and set flag
    engine.process_key(&press('a'));
    engine.metrics.adaptive_use_light_model = true;

    // Commit
    engine.process_key(&press_key(Keysym::RETURN));
    assert!(matches!(engine.state(), InputState::Empty));
    // Flag is still true (reset happens on next key in Empty state)
    assert!(engine.metrics.adaptive_use_light_model);

    engine.process_key(&press('k'));
    assert!(!engine.metrics.adaptive_use_light_model);
}

#[test]
fn test_config_default_max_latency_ms() {
    let config = EngineConfig::default();
    assert_eq!(config.max_latency_ms, 100);
}

#[test]
fn space_beam_is_independent_of_live_fallback_and_length_in_all_modes() {
    use crate::config::settings::StrategyMode;
    for mode in [
        StrategyMode::Adaptive,
        StrategyMode::Main,
        StrategyMode::Light,
    ] {
        for has_light in [false, true] {
            for adaptive in [false, true] {
                for tokens in [1, 10, 11, 100] {
                    let mut config = default_strategy_config();
                    config.strategy = mode;
                    assert_eq!(
                        determine_conversion_strategy(tokens, 9, has_light, adaptive, &config),
                        ConversionStrategy::MainModelBeam { beam_width: 3 }
                    );
                }
            }
        }
    }
}

#[test]
fn beam_latency_does_not_set_live_fallback() {
    let mut engine = InputMethodEngine::new();
    engine
        .init_light_kanji_converter("jinen-v2-xsmall-q5", 4)
        .expect("real fallback model");
    engine.metrics.conversion_ms = engine.config.max_latency_ms + 1;
    engine.update_adaptive_model_flag(&ConversionStrategy::MainModelBeam { beam_width: 3 });
    assert!(!engine.metrics.adaptive_use_light_model);
    engine.update_adaptive_model_flag(&ConversionStrategy::MainModelOnly);
    assert!(engine.metrics.adaptive_use_light_model);
}
