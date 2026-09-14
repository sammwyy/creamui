use std::marker::PhantomData;

/// A generational handle into an [`Arena`]. A stale `Id` (its slot was
/// removed, possibly reused since) is detected via generation mismatch
/// instead of aliasing whatever now occupies that slot.
pub struct Id<T> {
    index: u32,
    generation: u32,
    _marker: PhantomData<fn() -> T>,
}

impl<T> Id<T> {
    #[cfg(test)]
    pub(crate) fn from_raw(index: u32, generation: u32) -> Self {
        Id {
            index,
            generation,
            _marker: PhantomData,
        }
    }
}

impl<T> Clone for Id<T> {
    fn clone(&self) -> Self {
        *self
    }
}
impl<T> Copy for Id<T> {}
impl<T> PartialEq for Id<T> {
    fn eq(&self, other: &Self) -> bool {
        self.index == other.index && self.generation == other.generation
    }
}
impl<T> Eq for Id<T> {}
impl<T> std::hash::Hash for Id<T> {
    fn hash<H: std::hash::Hasher>(&self, state: &mut H) {
        self.index.hash(state);
        self.generation.hash(state);
    }
}
impl<T> std::fmt::Debug for Id<T> {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "Id({}, gen {})", self.index, self.generation)
    }
}

enum Slot<T> {
    Occupied { generation: u32, value: T },
    Vacant { generation: u32 },
}

/// A generational arena over freed-slot-reusing storage.
pub struct Arena<T> {
    slots: Vec<Slot<T>>,
    free: Vec<u32>,
}

impl<T> Default for Arena<T> {
    fn default() -> Self {
        Arena {
            slots: Vec::new(),
            free: Vec::new(),
        }
    }
}

impl<T> Arena<T> {
    pub fn new() -> Self {
        Self::default()
    }

    /// Inserts a value built from the [`Id`] it will be stored under —
    /// [`super::node::RuntimeNode`] carries its own `id`.
    pub fn insert_with(&mut self, build: impl FnOnce(Id<T>) -> T) -> Id<T> {
        if let Some(index) = self.free.pop() {
            let generation = match &self.slots[index as usize] {
                Slot::Vacant { generation } => *generation,
                Slot::Occupied { .. } => unreachable!("free-listed slot must be Vacant"),
            };
            let id = Id {
                index,
                generation,
                _marker: PhantomData,
            };
            self.slots[index as usize] = Slot::Occupied {
                generation,
                value: build(id),
            };
            id
        } else {
            let index = self.slots.len() as u32;
            let id = Id {
                index,
                generation: 0,
                _marker: PhantomData,
            };
            self.slots.push(Slot::Occupied {
                generation: 0,
                value: build(id),
            });
            id
        }
    }

    pub fn get(&self, id: Id<T>) -> Option<&T> {
        match self.slots.get(id.index as usize)? {
            Slot::Occupied { generation, value } if *generation == id.generation => Some(value),
            _ => None,
        }
    }

    pub fn get_mut(&mut self, id: Id<T>) -> Option<&mut T> {
        match self.slots.get_mut(id.index as usize)? {
            Slot::Occupied { generation, value } if *generation == id.generation => Some(value),
            _ => None,
        }
    }

    pub fn contains(&self, id: Id<T>) -> bool {
        self.get(id).is_some()
    }

    pub fn remove(&mut self, id: Id<T>) -> Option<T> {
        let slot = self.slots.get_mut(id.index as usize)?;
        match slot {
            Slot::Occupied { generation, .. } if *generation == id.generation => {
                let next_generation = generation.wrapping_add(1);
                let Slot::Occupied { value, .. } = std::mem::replace(
                    slot,
                    Slot::Vacant {
                        generation: next_generation,
                    },
                ) else {
                    unreachable!()
                };
                self.free.push(id.index);
                Some(value)
            }
            _ => None,
        }
    }

    pub fn len(&self) -> usize {
        self.slots.len() - self.free.len()
    }

    pub fn is_empty(&self) -> bool {
        self.len() == 0
    }

    pub fn iter(&self) -> impl Iterator<Item = (Id<T>, &T)> {
        self.slots
            .iter()
            .enumerate()
            .filter_map(|(index, slot)| match slot {
                Slot::Occupied { generation, value } => Some((
                    Id {
                        index: index as u32,
                        generation: *generation,
                        _marker: PhantomData,
                    },
                    value,
                )),
                Slot::Vacant { .. } => None,
            })
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn insert_and_get_roundtrip() {
        let mut arena: Arena<&'static str> = Arena::new();
        let id = arena.insert_with(|_| "hello");
        assert_eq!(arena.get(id), Some(&"hello"));
    }

    #[test]
    fn removed_id_is_detectably_stale_even_after_slot_reuse() {
        let mut arena: Arena<u32> = Arena::new();
        let a = arena.insert_with(|_| 1);
        arena.remove(a);
        let b = arena.insert_with(|_| 2);

        assert_eq!(b.index, a.index);
        assert_ne!(b.generation, a.generation);
        assert_eq!(arena.get(a), None);
        assert_eq!(arena.get(b), Some(&2));
    }

    #[test]
    fn get_mut_allows_updating_in_place() {
        let mut arena: Arena<u32> = Arena::new();
        let id = arena.insert_with(|_| 1);
        *arena.get_mut(id).unwrap() = 42;
        assert_eq!(arena.get(id), Some(&42));
    }

    #[test]
    fn len_excludes_removed_slots() {
        let mut arena: Arena<u32> = Arena::new();
        let a = arena.insert_with(|_| 1);
        let _b = arena.insert_with(|_| 2);
        assert_eq!(arena.len(), 2);
        arena.remove(a);
        assert_eq!(arena.len(), 1);
    }

    #[test]
    fn iter_skips_removed_slots() {
        let mut arena: Arena<u32> = Arena::new();
        let a = arena.insert_with(|_| 1);
        let b = arena.insert_with(|_| 2);
        arena.remove(a);
        let remaining: Vec<_> = arena.iter().map(|(id, &v)| (id, v)).collect();
        assert_eq!(remaining, vec![(b, 2)]);
    }

    #[test]
    fn unknown_id_returns_none() {
        let arena: Arena<u32> = Arena::new();
        let bogus = Id::from_raw(7, 0);
        assert_eq!(arena.get(bogus), None);
    }
}
