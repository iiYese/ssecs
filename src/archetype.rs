use std::collections::HashMap;

use derive_more::{Deref, DerefMut};
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
    pub signature: ArchetypeType,
    pub entities: Vec<Entity>,
    pub columns: Vec<RwLock<Column>>,
    pub edges: HashMap<FieldId, ArchetypeEdge>,
}

impl From<ArchetypeType> for Archetype {
    fn from(signature: ArchetypeType) -> Self {
        Self {
            signature,
            entities: Default::default(),
            columns: Default::default(),
            edges: Default::default(),
        }
    }
}

#[derive(Clone, Debug, Default, PartialEq, Eq, Hash)]
pub(crate) struct ArchetypeType(SmallVec<[FieldId; ARCHETYPE_SAO]>);

impl ArchetypeType {
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
}

#[derive(Debug)]
pub(crate) struct Column {
    pub buffer: Vec<u8>,
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

    pub fn remove_chunk(&mut self, row: usize) {
        if row < self.buffer.len() / self.chunk_size {
            let (left, right) = self.buffer.split_at_mut((row + 1) * self.chunk_size);
            let end_chunk_start = right.len() - self.chunk_size;
            left[row * self.chunk_size..].swap_with_slice(&mut right[end_chunk_start..]);
        }
        self.buffer.drain(self.buffer.len() - self.chunk_size..);
    }
}
