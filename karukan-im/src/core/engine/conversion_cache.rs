use std::collections::VecDeque;

use super::ConversionStrategy;

pub(super) const CONVERSION_RESULT_CACHE_CAPACITY: usize = 128;

#[derive(Debug, Clone, PartialEq, Eq)]
pub(super) struct ConversionResultKey {
    pub reading: String,
    pub left_context: String,
    pub num_candidates: usize,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub(super) struct CachedConversionResult {
    pub candidates: Vec<String>,
    pub source_strategy: ConversionStrategy,
    pub source_model_name: String,
}

#[derive(Clone)]
pub(super) struct ConversionResultCache {
    inner: std::sync::Arc<std::sync::Mutex<CacheEntries>>,
}

struct CacheEntries {
    capacity: usize,
    entries: VecDeque<(ConversionResultKey, CachedConversionResult)>,
}

impl Default for ConversionResultCache {
    fn default() -> Self {
        Self::with_capacity(CONVERSION_RESULT_CACHE_CAPACITY)
    }
}

impl ConversionResultCache {
    fn with_capacity(capacity: usize) -> Self {
        Self {
            inner: std::sync::Arc::new(std::sync::Mutex::new(CacheEntries {
                capacity,
                entries: VecDeque::with_capacity(capacity),
            })),
        }
    }

    pub fn get(
        &mut self,
        key: &ConversionResultKey,
        current_strategy: &ConversionStrategy,
    ) -> Option<CachedConversionResult> {
        let mut cache = self.inner.lock().unwrap();
        let index = cache
            .entries
            .iter()
            .position(|(candidate_key, _)| candidate_key == key)?;
        if !can_reuse_for_strategy(
            &cache.entries.get(index)?.1.source_strategy,
            current_strategy,
        ) {
            return None;
        }
        let entry = cache.entries.remove(index)?;
        let result = entry.1.clone();
        cache.entries.push_back(entry);
        Some(result)
    }

    pub fn insert(
        &mut self,
        key: ConversionResultKey,
        candidates: Vec<String>,
        source_strategy: ConversionStrategy,
        source_model_name: String,
    ) {
        let mut cache = self.inner.lock().unwrap();
        if cache.capacity == 0 {
            return;
        }
        if let Some(index) = cache
            .entries
            .iter()
            .position(|(candidate_key, _)| candidate_key == &key)
        {
            cache.entries.remove(index);
        } else if cache.entries.len() == cache.capacity {
            cache.entries.pop_front();
        }
        cache.entries.push_back((
            key,
            CachedConversionResult {
                candidates,
                source_strategy,
                source_model_name,
            },
        ));
    }
}

fn can_reuse_for_strategy(
    source_strategy: &ConversionStrategy,
    current_strategy: &ConversionStrategy,
) -> bool {
    source_strategy == current_strategy
}

#[cfg(test)]
mod tests {
    use super::*;

    fn key(reading: &str) -> ConversionResultKey {
        ConversionResultKey {
            reading: reading.to_string(),
            left_context: String::new(),
            num_candidates: 9,
        }
    }

    #[test]
    fn hit_promotes_entry_before_eviction() {
        let mut cache = ConversionResultCache::with_capacity(2);
        cache.insert(
            key("a"),
            vec!["A".to_string()],
            ConversionStrategy::MainModelBeam { beam_width: 3 },
            "main+light".to_string(),
        );
        cache.insert(
            key("b"),
            vec!["B".to_string()],
            ConversionStrategy::MainModelBeam { beam_width: 3 },
            "main+light".to_string(),
        );

        assert_eq!(
            cache
                .get(
                    &key("a"),
                    &ConversionStrategy::MainModelBeam { beam_width: 3 },
                )
                .map(|result| result.candidates),
            Some(vec!["A".to_string()])
        );
        cache.insert(
            key("c"),
            vec!["C".to_string()],
            ConversionStrategy::MainModelBeam { beam_width: 3 },
            "main+light".to_string(),
        );

        assert_eq!(
            cache.get(
                &key("b"),
                &ConversionStrategy::MainModelBeam { beam_width: 3 },
            ),
            None
        );
        assert_eq!(
            cache
                .get(
                    &key("a"),
                    &ConversionStrategy::MainModelBeam { beam_width: 3 },
                )
                .map(|result| result.candidates),
            Some(vec!["A".to_string()])
        );
        assert_eq!(
            cache
                .get(
                    &key("c"),
                    &ConversionStrategy::MainModelBeam { beam_width: 3 },
                )
                .map(|result| result.candidates),
            Some(vec!["C".to_string()])
        );
    }

    #[test]
    fn replacing_entry_keeps_latest_candidates() {
        let mut cache = ConversionResultCache::with_capacity(2);
        cache.insert(
            key("a"),
            vec!["old".to_string()],
            ConversionStrategy::MainModelBeam { beam_width: 3 },
            "main+light".to_string(),
        );
        cache.insert(
            key("a"),
            vec!["new".to_string()],
            ConversionStrategy::MainModelOnly,
            "main".to_string(),
        );

        assert_eq!(
            cache.get(&key("a"), &ConversionStrategy::MainModelOnly),
            Some(CachedConversionResult {
                candidates: vec!["new".to_string()],
                source_strategy: ConversionStrategy::MainModelOnly,
                source_model_name: "main".to_string(),
            })
        );
    }

    #[test]
    fn default_capacity_and_non_strategy_key_dimensions_are_fixed() {
        let mut cache = ConversionResultCache::default();
        let key = key("あい");
        cache.insert(
            key.clone(),
            vec!["愛".to_string()],
            ConversionStrategy::MainModelBeam { beam_width: 3 },
            "main+light".to_string(),
        );

        assert_eq!(
            cache.inner.lock().unwrap().capacity,
            CONVERSION_RESULT_CACHE_CAPACITY
        );
        assert_eq!(cache.inner.lock().unwrap().capacity, 128);
        assert_eq!(
            cache.get(
                &ConversionResultKey {
                    reading: "あお".to_string(),
                    ..key.clone()
                },
                &ConversionStrategy::MainModelBeam { beam_width: 3 },
            ),
            None
        );
        assert_eq!(
            cache.get(
                &ConversionResultKey {
                    left_context: "前文".to_string(),
                    ..key.clone()
                },
                &ConversionStrategy::MainModelBeam { beam_width: 3 },
            ),
            None
        );
        assert_eq!(
            cache.get(
                &ConversionResultKey {
                    num_candidates: 8,
                    ..key.clone()
                },
                &ConversionStrategy::MainModelBeam { beam_width: 3 },
            ),
            None
        );
        assert_eq!(
            cache.get(&key, &ConversionStrategy::MainModelBeam { beam_width: 3 },),
            Some(CachedConversionResult {
                candidates: vec!["愛".to_string()],
                source_strategy: ConversionStrategy::MainModelBeam { beam_width: 3 },
                source_model_name: "main+light".to_string(),
            })
        );
    }

    #[test]
    fn beam_and_live_results_are_not_interchangeable() {
        let mut cache = ConversionResultCache::with_capacity(2);
        cache.insert(
            key("a"),
            vec!["A".to_string()],
            ConversionStrategy::MainModelBeam { beam_width: 3 },
            "main+light".to_string(),
        );
        assert!(
            cache
                .get(&key("a"), &ConversionStrategy::LightModelOnly)
                .is_none()
        );

        cache.insert(
            key("b"),
            vec!["B".to_string()],
            ConversionStrategy::LightModelOnly,
            "light".to_string(),
        );
        assert!(
            cache
                .get(
                    &key("b"),
                    &ConversionStrategy::MainModelBeam { beam_width: 3 },
                )
                .is_none()
        );
    }
}
