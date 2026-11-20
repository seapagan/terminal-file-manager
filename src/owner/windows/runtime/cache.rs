//! Bounded session caches, including negative results; no cache mutex on the UI.
use std::{
    collections::{HashMap, VecDeque},
    hash::Hash,
    time::{Duration, Instant},
};

pub(super) struct Cache<K, V> {
    values: HashMap<K, (V, Option<Instant>)>,
    order: VecDeque<K>,
    capacity: usize,
    ttl: Duration,
}

impl<K: Eq + Hash + Clone, V> Cache<K, V> {
    pub fn new(capacity: usize, ttl: Duration) -> Self {
        Self {
            values: HashMap::new(),
            order: VecDeque::new(),
            capacity,
            ttl,
        }
    }

    pub fn get(&self, key: &K, now: Instant) -> Option<&V> {
        self.values
            .get(key)
            .filter(|(_, inserted)| {
                inserted.is_some_and(|inserted| now.saturating_duration_since(inserted) < self.ttl)
            })
            .map(|(value, _)| value)
    }

    pub fn insert(&mut self, key: K, value: V, now: Instant) {
        if !self.values.contains_key(&key) {
            if self.values.len() == self.capacity
                && let Some(oldest) = self.order.pop_front()
            {
                self.values.remove(&oldest);
            }
            self.order.push_back(key.clone());
        }
        self.values.insert(key, (value, Some(now)));
    }

    // Stale values remain presentation fallbacks, never evidence of freshness.
    pub fn peek(&self, key: &K) -> Option<&V> {
        self.values.get(key).map(|(value, _)| value)
    }

    pub fn invalidate(&mut self) {
        for (_, inserted) in self.values.values_mut() {
            *inserted = None;
        }
    }

    pub fn clear(&mut self) {
        self.values.clear();
        self.order.clear();
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn eviction_expiry_and_replacement_keep_cache_memory_bounded() {
        let now = Instant::now();
        let mut cache = Cache::new(2, Duration::from_secs(30));
        for key in 0..1000 {
            cache.insert(key, key, now);
        }
        assert_eq!(cache.values.len(), 2);
        assert_eq!(cache.get(&997, now), None);
        assert_eq!(cache.get(&998, now), Some(&998));
        cache.insert(999, 123, now);
        assert_eq!(cache.get(&999, now), Some(&123));
        assert_eq!(cache.order.len(), 2);
        assert_eq!(cache.get(&999, now + Duration::from_secs(31)), None);
    }
}
