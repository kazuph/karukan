//! Conversion strategy determination and adaptive model selection
//!
//! Explicit conversion (Space key) always runs a beam search on the model in
//! the main slot — the first candidate and every lower-ranked candidate come
//! from the same beam. The adaptive light-model switch therefore applies only
//! to single-candidate conversion (live conversion / auto-suggest): it exists
//! to bound per-keystroke latency, not to source Space candidates. Reading
//! length likewise no longer selects a different Space strategy.

use tracing::debug;

use crate::config::settings::StrategyMode;

use super::*;

/// Pure function to determine conversion strategy from the requested
/// candidate count, adaptive flag, and configuration.
///
/// This is separated from `InputMethodEngine` to enable unit testing without model instances.
///
/// `adaptive_use_light_model` is set by the engine when the main model's last
/// conversion exceeded `max_latency_ms`. It is reset when a new word begins.
pub(super) fn determine_conversion_strategy(
    _reading_tokens: usize,
    num_candidates: usize,
    has_light_model: bool,
    adaptive_use_light_model: bool,
    config: &EngineConfig,
) -> ConversionStrategy {
    if num_candidates > 1 {
        // Explicit conversion (Space): beam search on the model in the main
        // slot. In Light mode that slot holds the light model, so this is
        // also the light-model beam path.
        return ConversionStrategy::MainModelBeam {
            beam_width: num_candidates.min(config.beam_width),
        };
    }
    match config.strategy {
        StrategyMode::Adaptive => {
            // Single-candidate conversion adapts on measured latency.
            if has_light_model && adaptive_use_light_model {
                ConversionStrategy::LightModelOnly
            } else {
                ConversionStrategy::MainModelOnly
            }
        }
        StrategyMode::Light | StrategyMode::Main => ConversionStrategy::MainModelOnly,
    }
}

impl InputMethodEngine {
    /// Determine the conversion strategy based on the requested candidate
    /// count and configuration. Falls back to `MainModelOnly` when the main
    /// model is not loaded.
    pub(super) fn determine_strategy(
        &self,
        reading: &str,
        num_candidates: usize,
    ) -> ConversionStrategy {
        let has_light_model = self.converters.light_kanji.is_some();

        if self.converters.kanji.is_none() {
            debug!("No kanji converter loaded, fallback to MainModelOnly");
            return ConversionStrategy::MainModelOnly;
        }
        if num_candidates > 1 && !karukan_engine::contains_kana(reading) {
            // Should be unreachable — run_kana_kanji_conversion skips
            // kana-free readings before consulting the strategy — but keep
            // the cheap guard so a direct caller cannot dispatch a beam on
            // input the model would hallucinate over.
            return ConversionStrategy::MainModelOnly;
        }

        determine_conversion_strategy(
            0,
            num_candidates,
            has_light_model,
            self.metrics.adaptive_use_light_model,
            &self.config,
        )
    }

    /// Update the adaptive model switching flag based on the strategy used and
    /// measured latency. Only updates when the main model was involved.
    pub(super) fn update_adaptive_model_flag(&mut self, strategy: &ConversionStrategy) {
        // Only Adaptive mode uses the adaptive flag
        if self.config.strategy != StrategyMode::Adaptive {
            return;
        }
        if self.config.max_latency_ms == 0 || self.converters.light_kanji.is_none() {
            return;
        }
        match strategy {
            ConversionStrategy::MainModelOnly => {
                self.metrics.adaptive_use_light_model =
                    self.metrics.conversion_ms > self.config.max_latency_ms;
            }
            ConversionStrategy::LightModelOnly | ConversionStrategy::MainModelBeam { .. } => {
                // Don't update — beam latency is not representative of the
                // single-candidate latency the flag exists to bound, and
                // light-model latency says nothing about the main model.
            }
        }
    }
}
