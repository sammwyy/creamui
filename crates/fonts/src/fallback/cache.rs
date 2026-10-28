use std::{borrow::Borrow, collections::HashMap, hash::Hash, time::Instant};

struct Entry<V> {
    value: V,
    bytes: usize,
    used: Instant,
}

pub(super) struct Lru<K, V> {
    entries: HashMap<K, Entry<V>>,
    bytes: usize,
    max_entries: usize,
    max_bytes: usize,
}

impl<K: Eq + Hash + Clone, V> Lru<K, V> {
    pub(super) fn new(max_entries: usize, max_bytes: usize) -> Self {
        Self {
            entries: HashMap::new(),
            bytes: 0,
            max_entries,
            max_bytes,
        }
    }

    pub(super) fn get<Q: Eq + Hash + ?Sized>(&mut self, key: &Q) -> Option<&V>
    where
        K: Borrow<Q>,
    {
        let entry = self.entries.get_mut(key)?;
        entry.used = Instant::now();
        Some(&entry.value)
    }

    pub(super) fn touch(&mut self, predicate: impl Fn(&V) -> bool) {
        if let Some(entry) = self
            .entries
            .values_mut()
            .find(|entry| predicate(&entry.value))
        {
            entry.used = Instant::now();
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
        self.entries.insert(
            key,
            Entry {
                value,
                bytes,
                used: Instant::now(),
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
