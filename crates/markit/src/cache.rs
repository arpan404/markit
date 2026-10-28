use std::{
    collections::{hash_map::RandomState, VecDeque},
    hash::{BuildHasher, Hash},
};

/// Small byte-budgeted LRU. Keys include the complete source, so collisions never
/// return another document. Oversized results are used but not retained.
pub(crate) struct Cache<K, V> {
    entries: VecDeque<(u64, K, V, usize)>,
    hasher: RandomState,
    weight: usize,
    limit: usize,
}

impl<K: PartialEq + Hash, V: Clone> Cache<K, V> {
    pub fn new(limit: usize) -> Self {
        Self {
            entries: VecDeque::new(),
            hasher: RandomState::new(),
            weight: 0,
            limit,
        }
    }
    #[cfg(test)]
    pub fn get(&mut self, key: &K) -> Option<V> {
        self.get_by(key, |candidate| candidate == key)
    }
    pub fn get_by<Q: Hash + ?Sized>(
        &mut self,
        key: &Q,
        mut matches: impl FnMut(&K) -> bool,
    ) -> Option<V> {
        // Repeated renders usually reuse the newest source. A direct equality
        // check avoids hashing every byte again on that common path.
        if let Some((_, candidate, value, _)) = self.entries.front() {
            if matches(candidate) {
                return Some(value.clone());
            }
        }
        if self.entries.len() < 2 {
            return None;
        }
        let hash = self.hasher.hash_one(key);
        let index = self
            .entries
            .iter()
            .enumerate()
            .skip(1)
            .find(|(_, (fingerprint, candidate, _, _))| *fingerprint == hash && matches(candidate))
            .map(|(index, _)| index)?;
        let entry = self.entries.remove(index)?;
        let value = entry.2.clone();
        self.entries.push_front(entry);
        Some(value)
    }
    pub fn remove(&mut self, key: &K) {
        if let Some(index) = self
            .entries
            .iter()
            .position(|(_, candidate, _, _)| candidate == key)
        {
            if let Some((_, _, _, weight)) = self.entries.remove(index) {
                self.weight -= weight;
            }
        }
    }
    pub fn insert(&mut self, key: K, value: V, weight: usize) {
        if weight > self.limit {
            return;
        }
        let hash = self.hasher.hash_one(&key);
        if let Some(index) = self
            .entries
            .iter()
            .position(|(fingerprint, candidate, _, _)| *fingerprint == hash && candidate == &key)
        {
            if let Some((_, _, _, old)) = self.entries.remove(index) {
                self.weight -= old;
            }
        }
        while self.weight + weight > self.limit || self.entries.len() >= 128 {
            if let Some((_, _, _, old)) = self.entries.pop_back() {
                self.weight -= old;
            } else {
                break;
            }
        }
        self.weight += weight;
        self.entries.push_front((hash, key, value, weight));
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::{
        cell::Cell,
        hash::{Hash, Hasher},
        rc::Rc,
    };

    struct Key {
        id: usize,
        comparisons: Rc<Cell<usize>>,
        collide: bool,
    }
    impl PartialEq for Key {
        fn eq(&self, other: &Self) -> bool {
            self.comparisons.set(self.comparisons.get() + 1);
            self.id == other.id
        }
    }
    impl Hash for Key {
        fn hash<H: Hasher>(&self, state: &mut H) {
            if !self.collide {
                self.id.hash(state);
            }
        }
    }
    #[test]
    fn cache_misses_do_not_compare_every_source() {
        let comparisons = Rc::new(Cell::new(0));
        let mut cache = Cache::new(1024);
        for id in 0..64 {
            cache.insert(
                Key {
                    id,
                    comparisons: comparisons.clone(),
                    collide: false,
                },
                id,
                1,
            );
        }
        comparisons.set(0);
        assert_eq!(
            cache.get(&Key {
                id: 65,
                comparisons: comparisons.clone(),
                collide: false
            }),
            None
        );
        assert!(
            comparisons.get() < 4,
            "cache miss compared {} source keys",
            comparisons.get()
        );
    }
    #[test]
    fn collisions_still_compare_full_keys_and_lru_respects_budget() {
        let comparisons = Rc::new(Cell::new(0));
        let key = |id| Key {
            id,
            comparisons: comparisons.clone(),
            collide: true,
        };
        let mut cache = Cache::new(2);
        cache.insert(key(1), "one", 1);
        cache.insert(key(2), "two", 1);
        assert_eq!(cache.get(&key(1)), Some("one"));
        cache.insert(key(3), "three", 1);
        assert_eq!(cache.get(&key(2)), None);
        assert_eq!(cache.get(&key(1)), Some("one"));
        assert_eq!(cache.get(&key(3)), Some("three"));
        cache.insert(key(3), "oversized", 3);
        assert_eq!(cache.get(&key(3)), Some("three"));
    }
}
