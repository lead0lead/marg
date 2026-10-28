//! A least-recently-used cache with a byte budget, for rendered pages.
//!
//! Use is counted in frames: entries touched in the current frame are never
//! evicted, so the cache may go over budget rather than drop something that
//! is on screen.

use std::collections::HashMap;
use std::hash::Hash;

struct Entry<V> {
    value: V,
    bytes: usize,
    frame: u64,
}

pub struct ByteLru<K, V> {
    map: HashMap<K, Entry<V>>,
    total: usize,
    budget: usize,
    frame: u64,
}

impl<K: Hash + Eq + Clone, V> ByteLru<K, V> {
    pub fn new(budget: usize) -> Self {
        Self {
            map: HashMap::new(),
            total: 0,
            budget,
            frame: 1,
        }
    }

    /// Start a new frame: entries touched before now become evictable.
    pub fn next_frame(&mut self) {
        self.frame += 1;
    }

    pub fn set_budget(&mut self, budget: usize) {
        self.budget = budget;
        self.evict();
    }

    pub fn bytes(&self) -> usize {
        self.total
    }

    pub fn len(&self) -> usize {
        self.map.len()
    }

    pub fn is_empty(&self) -> bool {
        self.map.is_empty()
    }

    pub fn contains(&self, key: &K) -> bool {
        self.map.contains_key(key)
    }

    /// Get and mark as used in this frame.
    pub fn get(&mut self, key: &K) -> Option<&V> {
        let frame = self.frame;
        self.map.get_mut(key).map(|e| {
            e.frame = frame;
            &e.value
        })
    }

    /// All entries, without marking them used.
    pub fn iter(&self) -> impl Iterator<Item = (&K, &V)> {
        self.map.iter().map(|(k, e)| (k, &e.value))
    }

    /// Insert (marked as used in this frame), then evict the least recently
    /// used entries from earlier frames until the budget is met.
    pub fn insert(&mut self, key: K, value: V, bytes: usize) {
        let entry = Entry {
            value,
            bytes,
            frame: self.frame,
        };
        if let Some(old) = self.map.insert(key, entry) {
            self.total -= old.bytes;
        }
        self.total += bytes;
        self.evict();
    }

    /// Remove entries for which `keep` returns false.
    pub fn retain(&mut self, mut keep: impl FnMut(&K) -> bool) {
        let total = &mut self.total;
        self.map.retain(|k, e| {
            let k = keep(k);
            if !k {
                *total -= e.bytes;
            }
            k
        });
    }

    pub fn clear(&mut self) {
        self.map.clear();
        self.total = 0;
    }

    fn evict(&mut self) {
        while self.total > self.budget {
            let oldest = self
                .map
                .iter()
                .filter(|(_, e)| e.frame < self.frame)
                .min_by_key(|(_, e)| e.frame)
                .map(|(k, _)| k.clone());
            let Some(k) = oldest else { return };
            if let Some(e) = self.map.remove(&k) {
                self.total -= e.bytes;
            }
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn evicts_least_recently_used() {
        let mut c = ByteLru::new(30);
        c.insert("a", 1, 10);
        c.next_frame();
        c.insert("b", 2, 10);
        c.next_frame();
        c.insert("c", 3, 10);
        c.next_frame();
        c.get(&"a");
        c.insert("d", 4, 10);
        assert!(!c.contains(&"b"));
        assert!(c.contains(&"a") && c.contains(&"c") && c.contains(&"d"));
        assert_eq!(c.bytes(), 30);
    }

    #[test]
    fn never_evicts_entries_used_this_frame() {
        let mut c = ByteLru::new(15);
        c.insert("a", 1, 10);
        c.insert("b", 2, 10);
        assert_eq!(c.len(), 2);
        assert_eq!(c.bytes(), 20);
        c.next_frame();
        c.insert("c", 3, 1);
        // Both a and b are older than this frame; evicting one is enough.
        assert_eq!(c.len(), 2);
        assert!(c.bytes() <= 15);
    }

    #[test]
    fn replacing_updates_byte_count() {
        let mut c = ByteLru::new(100);
        c.insert(1, (), 40);
        c.insert(1, (), 10);
        assert_eq!(c.bytes(), 10);
        c.retain(|_| false);
        assert_eq!(c.bytes(), 0);
        assert!(c.is_empty());
    }
}
