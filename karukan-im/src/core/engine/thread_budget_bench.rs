use anyhow::{Result, bail};
use serde::Serialize;
use std::time::Instant;

use crate::config::settings::{Settings, StrategyMode};
use crate::core::keycode::KeyModifiers;

use super::*;

const REQUEST_CANDIDATES: usize = 9;
const MAIN_MODEL_ID: &str = "jinen-v2-small-q5";
const LIGHT_MODEL_ID: &str = "jinen-v2-xsmall-q5";
const READING: &str = "なにがげんいんかわかる";
const LEFT_CONTEXT: &str = "今日はいい";

const fn thread_budget(main_threads: u32, light_threads: u32) -> ParallelBeamThreadBudget {
    match ParallelBeamThreadBudget::new(main_threads, light_threads) {
        Some(budget) => budget,
        None => panic!("benchmark thread budget must be valid"),
    }
}

const THREAD_BUDGETS: [ParallelBeamThreadBudget; 7] = [
    thread_budget(4, 4),
    thread_budget(3, 1),
    thread_budget(2, 2),
    thread_budget(1, 3),
    thread_budget(2, 1),
    thread_budget(1, 2),
    thread_budget(1, 1),
];

#[derive(Debug, Clone, Serialize, PartialEq, Eq)]
struct CandidateDisplay {
    candidates: Vec<String>,
    page_candidates: Vec<String>,
    cursor: Option<usize>,
    page_cursor: Option<usize>,
}

#[derive(Debug, Clone, Serialize, PartialEq, Eq)]
struct CandidateQualityBaseline {
    display: CandidateDisplay,
    return_commit: String,
}

#[derive(Debug, Clone, Serialize)]
pub struct ThreadBudgetMeasurement {
    pub main_threads: u32,
    pub light_threads: u32,
    pub run: usize,
    pub reading: String,
    pub left_context: String,
    pub wall_ms: u64,
    pub conversion_ms: u64,
    pub process_key_ms: u64,
    pub main_inference_ms: u64,
    pub light_inference_ms: u64,
    pub prefill_ms: u64,
    pub decode_ms: u64,
    pub candidates: Vec<String>,
    pub page_candidates: Vec<String>,
    pub cursor: Option<usize>,
    pub page_cursor: Option<usize>,
    pub show_candidates_matches_state: bool,
    pub return_commit: String,
    pub adaptive_use_light_model: bool,
    pub next_strategy: String,
    pub matches_4_4_candidate_quality: bool,
}

#[derive(Debug, Serialize)]
pub struct ThreadBudgetBenchmarkReport {
    pub schema_version: u32,
    pub load_mode: String,
    pub runs_per_budget: usize,
    pub model: String,
    pub light_model: String,
    pub num_candidates: usize,
    pub reading: String,
    pub left_context: String,
    pub candidate_quality_gate_passed: bool,
    pub measurements: Vec<ThreadBudgetMeasurement>,
}

#[derive(Debug, Clone, Serialize)]
pub struct ThreadBudgetOperationMeasurement {
    pub operation: String,
    pub forced_adaptive: Option<bool>,
    pub operation_strategy: Option<String>,
    pub main_threads: u32,
    pub light_threads: u32,
    pub run: usize,
    pub wall_ms: u64,
    pub conversion_ms: u64,
    pub process_key_ms: u64,
    pub main_inference_ms: u64,
    pub light_inference_ms: u64,
    pub prefill_ms: u64,
    pub decode_ms: u64,
    pub candidates: Vec<String>,
    pub page_candidates: Vec<String>,
    pub cursor: Option<usize>,
    pub page_cursor: Option<usize>,
    pub return_commit: String,
    pub adaptive_use_light_model: bool,
    pub next_strategy: String,
    pub matches_4_4_candidate_quality: bool,
}

#[derive(Debug, Serialize)]
pub struct ThreadBudgetOperationBenchmarkReport {
    pub schema_version: u32,
    pub load_mode: String,
    pub runs_per_budget: usize,
    pub model: String,
    pub light_model: String,
    pub num_candidates: usize,
    pub reading: String,
    pub left_context: String,
    pub candidate_quality_gate_passed: bool,
    pub measurements: Vec<ThreadBudgetOperationMeasurement>,
}

fn p1_settings() -> Settings {
    let mut settings = Settings::default();
    settings.conversion.strategy = StrategyMode::Adaptive;
    settings.conversion.num_candidates = REQUEST_CANDIDATES;
    settings.conversion.use_context = true;
    settings.conversion.max_context_length = 10;
    settings.conversion.composing_chunk_len = 30;
    settings.conversion.short_input_threshold = 10;
    settings.conversion.beam_width = 3;
    settings.conversion.max_latency_ms = 100;
    settings.conversion.model = Some(MAIN_MODEL_ID.to_string());
    settings.conversion.light_model = Some(LIGHT_MODEL_ID.to_string());
    settings.conversion.n_threads = 4;
    settings.conversion.live_conversion = false;
    settings.conversion.tab_skips_learning = false;
    settings.learning.enabled = true;
    settings.learning.max_entries = 10_000;
    settings
}

fn p1_engine() -> Result<InputMethodEngine> {
    let settings = p1_settings();
    let mut engine = InputMethodEngine::with_config(EngineConfig::from_settings(&settings));
    engine.init_kanji_converter_with_model(MAIN_MODEL_ID, 4)?;
    engine.init_light_kanji_converter(LIGHT_MODEL_ID, 4)?;
    Ok(engine)
}

fn display(candidates: &CandidateList) -> CandidateDisplay {
    CandidateDisplay {
        candidates: candidates
            .candidates()
            .iter()
            .map(|candidate| candidate.text.clone())
            .collect(),
        page_candidates: candidates
            .page_candidates()
            .iter()
            .map(|candidate| candidate.text.clone())
            .collect(),
        cursor: candidates.cursor(),
        page_cursor: candidates.page_cursor(),
    }
}

fn strategy_name(strategy: &ConversionStrategy) -> &'static str {
    match strategy {
        ConversionStrategy::LightModelOnly => "light_model_only",
        ConversionStrategy::MainModelOnly => "main_model_only",
        ConversionStrategy::MainModelBeam { .. } => "main_model_beam",
    }
}

fn prepare_thread_budget_operation(
    engine: &mut InputMethodEngine,
    budget: ParallelBeamThreadBudget,
) -> Result<()> {
    engine.reset();
    engine.conversion_result_cache = Default::default();
    engine.set_parallel_beam_thread_budget(Some(budget));
    engine.set_surrounding_context(LEFT_CONTEXT, "");
    engine.input_buf.text = READING.to_string();
    engine.input_buf.cursor_pos = READING.chars().count();
    engine.state = InputState::Composing {
        preedit: Preedit::new(),
        romaji_buffer: String::new(),
    };
    if !matches!(
        engine.determine_strategy(READING, REQUEST_CANDIDATES),
        ConversionStrategy::MainModelBeam { .. }
    ) {
        bail!("P4 operation benchmark reading must resolve to ParallelBeam");
    }

    let space = engine.process_key(&KeyEvent::press(Keysym::SPACE));
    let action_display = space
        .actions
        .iter()
        .find_map(|action| match action {
            EngineAction::ShowCandidates(candidates) => Some(display(candidates)),
            _ => None,
        })
        .ok_or_else(|| anyhow::anyhow!("P4 operation setup Space did not show candidates"))?;
    let state_display = display(
        engine
            .candidates()
            .ok_or_else(|| anyhow::anyhow!("P4 operation setup did not enter Conversion"))?,
    );
    if action_display != state_display {
        bail!("P4 operation setup ShowCandidates diverged from conversion state");
    }
    Ok(())
}

fn measure_thread_budget_operation(
    engine: &mut InputMethodEngine,
    budget: ParallelBeamThreadBudget,
    run: usize,
    operation: &str,
    forced_adaptive: Option<bool>,
) -> Result<(ThreadBudgetOperationMeasurement, CandidateQualityBaseline)> {
    prepare_thread_budget_operation(engine, budget)?;

    if let Some(adaptive) = forced_adaptive {
        engine.metrics.adaptive_use_light_model = adaptive;
        engine.conversion_result_cache = Default::default();
    }
    if operation == "shift_right" {
        let adaptive = forced_adaptive
            .ok_or_else(|| anyhow::anyhow!("ShiftRight requires forced adaptive state"))?;
        engine.process_key(&KeyEvent::new(
            Keysym::LEFT,
            KeyModifiers::new().with_shift(true),
            true,
        ));
        engine.metrics.adaptive_use_light_model = adaptive;
        engine.conversion_result_cache = Default::default();
    }

    let operation_strategy = forced_adaptive
        .map(|_| {
            let target = engine
                .input_buf
                .text
                .chars()
                .take(engine.input_buf.cursor_pos)
                .collect::<String>();
            let strategy = engine.determine_strategy(&target, REQUEST_CANDIDATES);
            let expected = ConversionStrategy::MainModelBeam {
                beam_width: engine.config.beam_width,
            };
            if strategy != expected {
                bail!("P4 operation forced strategy did not resolve as requested");
            }
            Ok(strategy_name(&strategy).to_string())
        })
        .transpose()?;

    let key = match operation {
        "plain_left" => KeyEvent::press(Keysym::LEFT),
        "plain_right" => KeyEvent::press(Keysym::RIGHT),
        "shift_left" => KeyEvent::new(Keysym::LEFT, KeyModifiers::new().with_shift(true), true),
        "shift_right" => KeyEvent::new(Keysym::RIGHT, KeyModifiers::new().with_shift(true), true),
        _ => bail!("P4 operation benchmark received an unknown operation"),
    };
    let started = Instant::now();
    let result = engine.process_key(&key);
    let wall_ms = started.elapsed().as_millis() as u64;
    if !result.consumed {
        bail!("P4 operation benchmark key was not consumed");
    }
    if operation.starts_with("plain_") && engine.metrics.conversion_ms != 0 {
        bail!("P4 plain operation unexpectedly ran model inference");
    }
    let display = display(
        engine
            .candidates()
            .ok_or_else(|| anyhow::anyhow!("P4 operation did not retain candidates"))?,
    );
    let conversion_ms = engine.metrics.conversion_ms;
    let process_key_ms = engine.metrics.process_key_ms;
    let main_inference_ms = engine.metrics.main_inference_ms;
    let light_inference_ms = engine.metrics.light_inference_ms;
    let prefill_ms = engine.metrics.prefill_ms;
    let decode_ms = engine.metrics.decode_ms;
    let adaptive_use_light_model = engine.metrics.adaptive_use_light_model;
    let target = engine
        .input_buf
        .text
        .chars()
        .take(engine.input_buf.cursor_pos)
        .collect::<String>();
    let next_strategy = strategy_name(&engine.determine_strategy(&target, REQUEST_CANDIDATES));

    let commit = engine.process_key(&KeyEvent::press(Keysym::RETURN));
    let return_commit = commit
        .actions
        .iter()
        .find_map(|action| match action {
            EngineAction::Commit(text) => Some(text.clone()),
            _ => None,
        })
        .ok_or_else(|| anyhow::anyhow!("P4 operation Return did not commit"))?;
    let baseline = CandidateQualityBaseline {
        display: display.clone(),
        return_commit: return_commit.clone(),
    };

    Ok((
        ThreadBudgetOperationMeasurement {
            operation: operation.to_string(),
            forced_adaptive,
            operation_strategy,
            main_threads: budget.main_threads(),
            light_threads: budget.light_threads(),
            run,
            wall_ms,
            conversion_ms,
            process_key_ms,
            main_inference_ms,
            light_inference_ms,
            prefill_ms,
            decode_ms,
            candidates: display.candidates,
            page_candidates: display.page_candidates,
            cursor: display.cursor,
            page_cursor: display.page_cursor,
            return_commit,
            adaptive_use_light_model,
            next_strategy: next_strategy.to_string(),
            matches_4_4_candidate_quality: false,
        },
        baseline,
    ))
}

fn measure(
    engine: &mut InputMethodEngine,
    budget: ParallelBeamThreadBudget,
    run: usize,
) -> Result<(ThreadBudgetMeasurement, CandidateQualityBaseline)> {
    engine.reset();
    engine.conversion_result_cache = Default::default();
    engine.set_parallel_beam_thread_budget(Some(budget));
    engine.set_surrounding_context(LEFT_CONTEXT, "");
    engine.input_buf.text = READING.to_string();
    engine.input_buf.cursor_pos = READING.chars().count();
    engine.state = InputState::Composing {
        preedit: Preedit::new(),
        romaji_buffer: String::new(),
    };

    if !matches!(
        engine.determine_strategy(READING, REQUEST_CANDIDATES),
        ConversionStrategy::MainModelBeam { .. }
    ) {
        bail!("P4 benchmark reading must resolve to ParallelBeam");
    }

    let started = Instant::now();
    let space = engine.process_key(&KeyEvent::press(Keysym::SPACE));
    let wall_ms = started.elapsed().as_millis() as u64;
    let action_display = space
        .actions
        .iter()
        .find_map(|action| match action {
            EngineAction::ShowCandidates(candidates) => Some(display(candidates)),
            _ => None,
        })
        .ok_or_else(|| anyhow::anyhow!("P4 benchmark Space did not show candidates"))?;
    let state_display = display(
        engine
            .candidates()
            .ok_or_else(|| anyhow::anyhow!("P4 benchmark Space did not enter Conversion"))?,
    );
    let show_candidates_matches_state = action_display == state_display;
    if !show_candidates_matches_state {
        bail!("P4 benchmark ShowCandidates diverged from conversion state");
    }

    let adaptive_use_light_model = engine.metrics.adaptive_use_light_model;
    let next_strategy = strategy_name(&engine.determine_strategy(READING, REQUEST_CANDIDATES));
    let conversion_ms = engine.metrics.conversion_ms;
    let process_key_ms = engine.metrics.process_key_ms;
    let main_inference_ms = engine.metrics.main_inference_ms;
    let light_inference_ms = engine.metrics.light_inference_ms;
    let prefill_ms = engine.metrics.prefill_ms;
    let decode_ms = engine.metrics.decode_ms;

    let commit = engine.process_key(&KeyEvent::press(Keysym::RETURN));
    let return_commit = commit
        .actions
        .iter()
        .find_map(|action| match action {
            EngineAction::Commit(text) => Some(text.clone()),
            _ => None,
        })
        .ok_or_else(|| anyhow::anyhow!("P4 benchmark Return did not commit"))?;
    let baseline = CandidateQualityBaseline {
        display: action_display.clone(),
        return_commit: return_commit.clone(),
    };

    Ok((
        ThreadBudgetMeasurement {
            main_threads: budget.main_threads(),
            light_threads: budget.light_threads(),
            run,
            reading: READING.to_string(),
            left_context: LEFT_CONTEXT.to_string(),
            wall_ms,
            conversion_ms,
            process_key_ms,
            main_inference_ms,
            light_inference_ms,
            prefill_ms,
            decode_ms,
            candidates: action_display.candidates,
            page_candidates: action_display.page_candidates,
            cursor: action_display.cursor,
            page_cursor: action_display.page_cursor,
            show_candidates_matches_state,
            return_commit,
            adaptive_use_light_model,
            next_strategy: next_strategy.to_string(),
            matches_4_4_candidate_quality: false,
        },
        baseline,
    ))
}

pub fn run_thread_budget_benchmark(
    runs_per_budget: usize,
    load_mode: String,
) -> Result<ThreadBudgetBenchmarkReport> {
    if runs_per_budget == 0 {
        bail!("runs_per_budget must be positive");
    }

    let mut engine = p1_engine()?;
    let mut measurements = Vec::with_capacity(THREAD_BUDGETS.len() * runs_per_budget);
    let mut baseline = None;

    for run in 1..=runs_per_budget {
        for budget in THREAD_BUDGETS {
            let (mut measurement, observed) = measure(&mut engine, budget, run)?;
            let expected = baseline.get_or_insert(observed.clone());
            measurement.matches_4_4_candidate_quality = observed == *expected;
            measurements.push(measurement);
        }
    }

    let candidate_quality_gate_passed = measurements
        .iter()
        .all(|measurement| measurement.matches_4_4_candidate_quality);
    Ok(ThreadBudgetBenchmarkReport {
        schema_version: 1,
        load_mode,
        runs_per_budget,
        model: MAIN_MODEL_ID.to_string(),
        light_model: LIGHT_MODEL_ID.to_string(),
        num_candidates: REQUEST_CANDIDATES,
        reading: READING.to_string(),
        left_context: LEFT_CONTEXT.to_string(),
        candidate_quality_gate_passed,
        measurements,
    })
}

pub fn run_thread_budget_operation_benchmark(
    runs_per_budget: usize,
    load_mode: String,
) -> Result<ThreadBudgetOperationBenchmarkReport> {
    if runs_per_budget == 0 {
        bail!("runs_per_budget must be positive");
    }

    let mut engine = p1_engine()?;
    let mut measurements = Vec::with_capacity(12 * runs_per_budget);
    for run in 1..=runs_per_budget {
        for (operation, forced_adaptive) in [
            ("plain_left", None),
            ("plain_right", None),
            ("shift_left", Some(false)),
            ("shift_left", Some(true)),
            ("shift_right", Some(false)),
            ("shift_right", Some(true)),
        ] {
            let mut baseline = None;
            for budget in [thread_budget(4, 4), thread_budget(2, 2)] {
                let (mut measurement, observed) = measure_thread_budget_operation(
                    &mut engine,
                    budget,
                    run,
                    operation,
                    forced_adaptive,
                )?;
                let expected = baseline.get_or_insert(observed.clone());
                measurement.matches_4_4_candidate_quality = observed == *expected;
                measurements.push(measurement);
            }
        }
    }

    let candidate_quality_gate_passed = measurements
        .iter()
        .all(|measurement| measurement.matches_4_4_candidate_quality);
    Ok(ThreadBudgetOperationBenchmarkReport {
        schema_version: 1,
        load_mode,
        runs_per_budget,
        model: MAIN_MODEL_ID.to_string(),
        light_model: LIGHT_MODEL_ID.to_string(),
        num_candidates: REQUEST_CANDIDATES,
        reading: READING.to_string(),
        left_context: LEFT_CONTEXT.to_string(),
        candidate_quality_gate_passed,
        measurements,
    })
}
