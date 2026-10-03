//! Debounced Space beam. Only finished results from the current input
//! generation enter the shared conversion cache; no provisional UI is emitted.

use std::sync::{Arc, Condvar, Mutex};
use std::thread::JoinHandle;
use std::time::{Duration, Instant};

use super::conversion_cache::{ConversionResultCache, ConversionResultKey};
use super::*;
use karukan_engine::KanaKanjiConverter;

struct Job {
    generation: u64,
    key: ConversionResultKey,
    beam_width: usize,
    deadline: Instant,
}

#[derive(Default)]
struct Shared {
    shutdown: bool,
    generation: u64,
    pending: Option<Job>,
    running: Option<(u64, ConversionResultKey)>,
}

pub(super) struct SpacePrefetcher {
    shared: Arc<(Mutex<Shared>, Condvar)>,
    worker: Option<JoinHandle<()>>,
}

impl SpacePrefetcher {
    fn new(converter: Arc<KanaKanjiConverter>, mut cache: ConversionResultCache) -> Self {
        let shared = Arc::new((Mutex::new(Shared::default()), Condvar::new()));
        let worker_shared = Arc::clone(&shared);
        let worker = std::thread::Builder::new()
            .name("karukan-space-prefetch".into())
            .spawn(move || {
                let (lock, wake) = &*worker_shared;
                let mut state = lock.lock().unwrap();
                loop {
                    if state.shutdown {
                        return;
                    }
                    let Some(job) = state.pending.as_ref() else {
                        state = wake.wait(state).unwrap();
                        continue;
                    };
                    if let Some(wait) = job.deadline.checked_duration_since(Instant::now()) {
                        state = wake.wait_timeout(state, wait).unwrap().0;
                        continue;
                    }
                    let job = state.pending.take().unwrap();
                    state.running = Some((job.generation, job.key.clone()));
                    drop(state);
                    let result = converter.convert(
                        &karukan_engine::hiragana_to_katakana(&job.key.reading),
                        &job.key.left_context,
                        job.beam_width,
                    );
                    state = lock.lock().unwrap();
                    if state.generation == job.generation
                        && !state.shutdown
                        && let Ok(candidates) = result
                    {
                        cache.insert(
                            job.key,
                            candidates,
                            ConversionStrategy::MainModelBeam {
                                beam_width: job.beam_width,
                            },
                            converter.model_display_name().to_string(),
                        );
                    }
                    state.running = None;
                    wake.notify_all();
                }
            })
            .expect("space prefetch worker must spawn");
        Self {
            shared,
            worker: Some(worker),
        }
    }

    fn schedule(&self, key: ConversionResultKey, beam_width: usize, delay: Duration) {
        let (lock, wake) = &*self.shared;
        let mut state = lock.lock().unwrap();
        state.generation += 1;
        state.pending = Some(Job {
            generation: state.generation,
            key,
            beam_width,
            deadline: Instant::now() + delay,
        });
        wake.notify_all();
    }

    pub fn invalidate(&self) {
        let (lock, wake) = &*self.shared;
        let mut state = lock.lock().unwrap();
        state.generation += 1;
        state.pending = None;
        wake.notify_all();
    }

    #[cfg(test)]
    pub fn wait_until_idle(&self) {
        let (lock, wake) = &*self.shared;
        let mut state = lock.lock().unwrap();
        while state.pending.is_some() || state.running.is_some() {
            state = wake.wait(state).unwrap();
        }
    }

    /// Bring a matching beam forward and serialize Space with any running
    /// beam, including an obsolete generation. A stale call cannot be
    /// cancelled inside llama.cpp, so wait instead of oversubscribing it.
    pub fn wait_for(&self, key: &ConversionResultKey) {
        let (lock, wake) = &*self.shared;
        let mut state = lock.lock().unwrap();
        if let Some(job) = state.pending.as_mut() {
            if &job.key == key {
                job.deadline = Instant::now();
            } else {
                // A different target (e.g. resized conversion) must not
                // start a background beam while Space computes inline.
                state.pending = None;
            }
            wake.notify_all();
        }
        while !state.shutdown
            && (state.pending.as_ref().is_some_and(|job| &job.key == key)
                || state.running.is_some())
        {
            state = wake.wait(state).unwrap();
        }
    }
}

impl Drop for SpacePrefetcher {
    fn drop(&mut self) {
        let (lock, wake) = &*self.shared;
        {
            let mut state = lock.lock().unwrap();
            state.shutdown = true;
            wake.notify_all();
        }
        if let Some(worker) = self.worker.take() {
            let _ = worker.join();
        }
    }
}

impl InputMethodEngine {
    pub(super) fn invalidate_space_prefetch(&self) {
        if let Some(worker) = &self.space_prefetcher {
            worker.invalidate();
        }
    }

    pub(super) fn schedule_space_prefetch(&mut self) {
        if self.config.prefetch_delay_ms == 0
            || self.live.enabled
            || !matches!(self.state, InputState::Composing { .. })
            || !matches!(self.input_mode, InputMode::Hiragana | InputMode::Katakana)
        {
            self.invalidate_space_prefetch();
            return;
        }
        let Some(converter) = &self.converters.kanji else {
            return;
        };
        // Replaying just the unresolved romaji buffer reproduces Space's flush
        // without changing the composing state or its raw key history.
        let mut romaji = RomajiConverter::new();
        for ch in self.converters.romaji.buffer().chars() {
            romaji.push(ch);
        }
        romaji.flush();
        let mut reading = self.input_buf.text.clone();
        let offset = reading
            .char_indices()
            .nth(self.input_buf.cursor_pos)
            .map_or(reading.len(), |(offset, _)| offset);
        reading.insert_str(offset, romaji.output());
        if !karukan_engine::contains_kana(&reading) {
            return;
        }
        let key = ConversionResultKey {
            reading,
            left_context: self.truncate_context_for_api(),
            num_candidates: self.config.num_candidates,
        };
        let beam_width = self.config.num_candidates.min(self.config.beam_width);
        let strategy = ConversionStrategy::MainModelBeam { beam_width };
        if self.conversion_result_cache.get(&key, &strategy).is_some() {
            return;
        }
        let worker = self.space_prefetcher.get_or_insert_with(|| {
            SpacePrefetcher::new(Arc::clone(converter), self.conversion_result_cache.clone())
        });
        worker.schedule(
            key,
            beam_width,
            Duration::from_millis(self.config.prefetch_delay_ms),
        );
    }
}
