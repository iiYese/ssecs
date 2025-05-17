use std::collections::HashMap;

use parking_lot::RwLock;
use slotmap::{KeyData, new_key_type};
use smallvec::SmallVec;

use crate::entity::Entity;

const ARCHETYPE_SAO: usize = 8;

new_key_type! { pub(crate) struct ArchetypeId; }

impl ArchetypeId {
    pub(crate) fn null() -> Self {
        Self::default()
    }

    pub(crate) fn empty_archetype() -> ArchetypeId {
        Self(KeyData::from_ffi(1))
    }
}

/// Component or pair
#[derive(Clone, Copy, Debug, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub struct FieldId(pub u64);

impl From<Entity> for FieldId {
    fn from(entity: Entity) -> Self {
        Self(entity.raw() & u32::MAX as u64)
    }
}

#[derive(Clone, Copy, Debug, Default)]
pub(crate) struct ArchetypeEdge {
    pub add: ArchetypeId,
    pub remove: ArchetypeId,
}

#[derive(Debug, Default)]
pub(crate) struct Archetype {
    pub signature: Signature,
    pub entities: Vec<Entity>,
    pub columns: Vec<RwLock<Column>>,
    pub edges: HashMap<FieldId, ArchetypeEdge>,
}

#[derive(Clone, Debug, Default, PartialEq, Eq, Hash)]
pub(crate) struct Signature(SmallVec<[FieldId; ARCHETYPE_SAO]>);

impl Signature {
    pub fn new(fields: &[FieldId]) -> Self {
        let mut fields = SmallVec::from(fields);
        fields.sort();
        fields.dedup();
        Self(fields)
    }

    pub fn contains(&self, field: FieldId) -> bool {
        self.0.contains(&field)
    }

    pub fn with(mut self, field: FieldId) -> Self {
        if let Err(n) = self.0.binary_search(&field) {
            self.0.insert(n, field);
        }
        self
    }

    pub fn without(mut self, field: FieldId) -> Self {
        if let Ok(n) = self.0.binary_search(&field) {
            self.0.remove(n);
        };
        self
    }

    pub fn iter(&self) -> impl Iterator<Item = &FieldId> {
        self.0.iter()
    }

    pub fn each_shared(&self, other: &Self, mut func: impl FnMut(usize, usize)) {
        if self.0.is_empty() || other.0.is_empty() {
            return;
        }
        let [mut n, mut m] = [0; 2];
        while n < self.0.len() && self.0[n] < other.0[m] {
            n += 1;
        }
        if n == self.0.len() {
            return;
        }
        while m < other.0.len() && other.0[m] < self.0[n] {
            m += 1;
        }
        if m == other.0.len() {
            return;
        }
        while n < self.0.len() && m < other.0.len() {
            if self.0[n] == other.0[m] {
                func(n, m);
            }
            if self.0[n] < other.0[m] {
                n += 1;
            } else {
                m += 1;
            }
        }
    }
}

#[derive(Debug)]
pub(crate) struct Column {
    buffer: Vec<u8>,
    chunk_size: usize,
}

impl Column {
    pub fn new(chunk_size: usize) -> Self {
        Self {
            buffer: Vec::new(),
            chunk_size,
        }
    }

    pub fn get_chunk(&self, row: usize) -> &[u8] {
        &self.buffer[row * self.chunk_size..][..self.chunk_size]
    }

    pub fn insert_chunk(&mut self, row: usize, bytes: &[u8]) {
        debug_assert_eq!(bytes.len(), self.chunk_size);
        debug_assert!(row < self.buffer.len() / self.chunk_size);
        self.buffer[row * bytes.len()..].copy_from_slice(bytes);
    }

    pub fn push_chunk(&mut self, bytes: &[u8]) {
        debug_assert_eq!(bytes.len(), self.chunk_size);
        self.buffer.extend_from_slice(bytes)
    }

    pub fn remove_chunk(&mut self, row: usize) -> impl Iterator<Item = u8> {
        if row < self.buffer.len() / self.chunk_size {
            let (left, right) = self.buffer.split_at_mut((row + 1) * self.chunk_size);
            let end_chunk_start = right.len() - self.chunk_size;
            left[row * self.chunk_size..].swap_with_slice(&mut right[end_chunk_start..]);
        }
        self.buffer.drain(self.buffer.len() - self.chunk_size..)
    }

    pub fn extend_from_drained(&mut self, drained: impl Iterator<Item = u8>) {
        self.buffer.extend(drained);
        debug_assert!(self.buffer.len() % self.chunk_size == 0);
    }

    pub fn zero_fill(&mut self, target_chunks: usize) {
        self.buffer.resize(target_chunks * self.chunk_size, 0);
    }

    pub fn truncate(&mut self, target_chunks: usize) {
        self.buffer.truncate(target_chunks * self.chunk_size);
    }
}
