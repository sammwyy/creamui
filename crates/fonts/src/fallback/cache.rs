use std::{borrow::Borrow, collections::HashMap, hash::Hash};

struct Entry<V> {
    value: V,
    bytes: usize,
    used: u64,
}

pub(super) struct Lru<K, V> {
    entries: HashMap<K, Entry<V>>,
    bytes: usize,
    max_entries: usize,
    max_bytes: usize,
    clock: u64,
}

impl<K: Eq + Hash + Clone, V> Lru<K, V> {
    pub(super) fn new(max_entries: usize, max_bytes: usize) -> Self {
        Self {
            entries: HashMap::new(),
            bytes: 0,
            max_entries,
            max_bytes,
            clock: 0,
        }
    }

    pub(super) fn get<Q: Eq + Hash + ?Sized>(&mut self, key: &Q) -> Option<&V>
    where
        K: Borrow<Q>,
    {
        let entry = self.entries.get_mut(key)?;
        self.clock = self.clock.wrapping_add(1);
        entry.used = self.clock;
        Some(&entry.value)
    }

    pub(super) fn touch(&mut self, predicate: impl Fn(&V) -> bool) {
        if let Some(entry) = self
            .entries
            .values_mut()
            .find(|entry| predicate(&entry.value))
        {
            self.clock = self.clock.wrapping_add(1);
            entry.used = self.clock;
        }
    }

    pub(super) fn insert(&mut self, key: K, value: V, bytes: usize) {
        if bytes > self.max_bytes {
            return;
        }
        if let Some(previous) = self.entries.remove(&key) {
            self.bytes -= previous.bytes;
        }
        while self.entries.len() >= self.max_entries || self.bytes > self.max_bytes - bytes {
            let oldest = self
                .entries
                .iter()
                .min_by_key(|(_, entry)| entry.used)
                .unwrap()
                .0
                .clone();
            self.bytes -= self.entries.remove(&oldest).unwrap().bytes;
        }
        self.clock = self.clock.wrapping_add(1);
        self.entries.insert(
            key,
            Entry {
                value,
                bytes,
                used: self.clock,
            },
        );
        self.bytes += bytes;
    }

    #[cfg(test)]
    pub(super) fn len(&self) -> usize {
        self.entries.len()
    }

    #[cfg(test)]
    pub(super) fn bytes(&self) -> usize {
        self.bytes
    }
}

#[cfg(test)]
mod tests {
    use super::Lru;

    #[test]
    fn evicts_least_recently_used_after_get_and_touch() {
        let mut cache = Lru::new(2, 4);
        cache.insert("first", 1, 2);
        cache.insert("second", 2, 2);
        assert_eq!(cache.get(&"first"), Some(&1));
        cache.insert("third", 3, 2);
        assert_eq!(cache.get(&"second"), None);
        cache.touch(|value| *value == 1);
        cache.insert("fourth", 4, 2);
        assert_eq!(cache.get(&"third"), None);
        assert_eq!(cache.get(&"first"), Some(&1));
        assert_eq!(cache.bytes(), 4);
        assert_eq!(cache.len(), 2);
    }
}
