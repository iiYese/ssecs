use std::{
    marker::PhantomData,
    ops::{Index, IndexMut},
};

#[derive(Clone, Copy, Debug, Default, PartialEq, Eq, Hash)]
pub(crate) struct Key {
    pub(crate) index: u32,
    pub(crate) generation: u32,
}

impl Key {
    pub fn raw(self) -> u64 {
        (u64::from(self.generation) << 32) | u64::from(self.index)
    }

    pub fn from_raw(value: u64) -> Self {
        Self { index: (value & 0xffff_ffff) as u32, generation: (value >> 32) as u32 }
    }
}

#[derive(Clone, Copy)]
pub(crate) struct Slot<T> {
    pub(crate) generation: u32, // 0_u32: NULL (unused index)
    pub(crate) data: Option<T>,
}

impl<T> Default for Slot<T> {
    fn default() -> Self {
        Self { generation: 0, data: None }
    }
}

#[derive(Clone)]
pub(crate) struct SlotMap<K, T> {
    pub(crate) slots: Vec<Slot<T>>,
    pub(crate) available: Vec<usize>,
    _phantom: PhantomData<K>,
}

impl<K, T> Index<K> for SlotMap<K, T>
where
    K: Copy + From<Key>,
    Key: From<K>,
{
    type Output = T;
    fn index(&self, index: K) -> &Self::Output {
        self.get(index).unwrap()
    }
}

impl<K, T> IndexMut<K> for SlotMap<K, T>
where
    K: Copy + From<Key>,
    Key: From<K>,
{
    fn index_mut(&mut self, index: K) -> &mut Self::Output {
        self.get_mut(index).unwrap()
    }
}

impl<K, T> Default for SlotMap<K, T> {
    fn default() -> Self {
        Self { slots: Vec::new(), available: Vec::new(), _phantom: PhantomData }
    }
}

impl<K, T> SlotMap<K, T>
where
    K: Copy + From<Key>,
    Key: From<K>,
{
    /// Returns `None` if there are no more slots left
    pub fn insert(&mut self, data: T) -> K {
        let slot_index = if let Some(index) = self.available.pop() {
            index
        } else {
            if u32::MAX as usize == self.slots.len() {
                panic!("Reached slotmap limit");
            }
            self.slots.push(Slot::default());
            self.slots.len() - 1
        };
        let slot = &mut self.slots[slot_index];
        slot.data = Some(data);
        slot.generation = if slot.generation != u32::MAX {
            slot.generation + 1
        } else {
            1
        };
        K::from(Key { index: slot_index as u32, generation: slot.generation })
    }

    pub fn remove(&mut self, key: K) -> Option<T> {
        let key = Key::from(key);
        self.slots
            .get_mut(key.index as usize)
            .filter(|slot| slot.generation == key.generation)
            .and_then(|slot| slot.data.take())
    }

    pub fn remove_ignore_generation(&mut self, key: K) -> Option<T> {
        self.slots.get_mut(Key::from(key).index as usize).and_then(|slot| slot.data.take())
    }

    pub fn get(&self, key: K) -> Option<&T> {
        let key = Key::from(key);
        self.slots
            .get(key.index as usize)
            .filter(|slot| slot.generation == key.generation)
            .and_then(|slot| slot.data.as_ref())
    }

    pub fn get_ignore_generation(&self, key: K) -> Option<&T> {
        self.slots.get(Key::from(key).index as usize).and_then(|slot| slot.data.as_ref())
    }

    pub fn get_mut(&mut self, key: K) -> Option<&mut T> {
        let key = Key::from(key);
        self.slots
            .get_mut(key.index as usize)
            .filter(|slot| slot.generation == key.generation)
            .and_then(|slot| slot.data.as_mut())
    }

    pub fn get_mut_ignore_generation(&mut self, key: K) -> Option<&mut T> {
        self.slots.get_mut(Key::from(key).index as usize).and_then(|slot| slot.data.as_mut())
    }

    pub fn disjoint<const N: usize>(&mut self, keys: [K; N]) -> Option<[&mut T; N]> {
        if keys.iter().any(|key| self.get(*key).is_none()) {
            return None;
        }
        self.slots
            .get_disjoint_mut(keys.map(|k| Key::from(k).index as usize))
            .map(|slots| slots.map(|slot| slot.data.as_mut().unwrap()))
            .ok()
    }
}
