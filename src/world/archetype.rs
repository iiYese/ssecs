use std::{collections::HashMap, mem::MaybeUninit};

use aligned_vec::{AVec, RuntimeAlign};
use derive_more::{Deref, DerefMut};
use parking_lot::RwLock;
use slotmap::{KeyData, new_key_type};
use smallvec::SmallVec;

use crate::{component::ComponentInfo, entity::Entity};

new_key_type! { pub(crate) struct ArchetypeId; }
const ARCHETYPE_SAO: usize = 8;

impl ArchetypeId {
    pub(crate) fn empty_archetype() -> ArchetypeId {
        Self(KeyData::from_ffi(1))
    }
}

#[derive(Clone, Copy, Deref, DerefMut, Debug)]
pub(crate) struct ColumnIndex(pub usize);

#[derive(Clone, Copy, Deref, DerefMut, Debug, PartialEq, Eq)]
pub(crate) struct RowIndex(pub usize);

#[derive(Debug, Default)]
pub(crate) struct Archetype {
    pub signature: Signature,
    pub entities: Vec<Entity>,
    pub columns: Vec<RwLock<Column>>,
    pub edges: HashMap<FieldId, ArchetypeEdge>,
}

impl Archetype {
    pub(crate) fn drop(&mut self, row: RowIndex) {
        self.entities.swap_remove(*row);
        for column in &mut self.columns {
            column.get_mut().swap_drop(row);
        }
    }
}

#[derive(Clone, Copy, Debug, Default)]
pub(crate) struct ArchetypeEdge {
    pub add: Option<ArchetypeId>,
    pub remove: Option<ArchetypeId>,
}

/// Component or pair
#[derive(Clone, Copy, Debug, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub struct FieldId(pub u64);

impl From<Entity> for FieldId {
    fn from(entity: Entity) -> Self {
        Self(entity.raw() & u32::MAX as u64)
    }
}

impl FieldId {
    // TODO: Check for pairs
    pub(crate) fn as_entity(&self) -> Option<Entity> {
        Some(Entity::from_ffi(self.0))
    }
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
        self.0.binary_search(&field).is_ok()
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
    buffer: AVec<MaybeUninit<u8>, RuntimeAlign>,
    info: ComponentInfo,
}

impl Column {
    pub fn new(component_info: ComponentInfo) -> Self {
        Self { buffer: AVec::new(component_info.align), info: component_info }
    }

    fn swap_with_last(&mut self, RowIndex(row): RowIndex) {
        if row + 1 < self.no_chunks() {
            let (left, right) = self.buffer.split_at_mut((row + 1) * self.info.size);
            left[row * self.info.size..].swap_with_slice(right);
        }
    }

    pub fn no_chunks(&self) -> usize {
        if self.info.size == 0 {
            0
        } else {
            self.buffer.len() / self.info.size
        }
    }

    pub fn get_chunk(&self, RowIndex(row): RowIndex) -> &[MaybeUninit<u8>] {
        &self.buffer[row * self.info.size..][..self.info.size]
    }

    pub unsafe fn write_into(&mut self, RowIndex(row): RowIndex, bytes: &[MaybeUninit<u8>]) {
        debug_assert_eq!(bytes.len(), self.info.size);
        if self.info.size == 0 {
            return;
        }
        if row < self.no_chunks() {
            // SAFETY: Chunk is written into
            unsafe { self.call_drop(RowIndex(row)) };
            self.buffer[row * self.info.size..].copy_from_slice(bytes);
        } else {
            self.buffer.extend_from_slice(bytes);
        }
    }

    pub fn move_into(&mut self, other: &mut Self, RowIndex(row): RowIndex) {
        debug_assert_eq!(self.info, other.info);
        if self.info.size == 0 {
            return;
        }

        // Swap with last
        self.swap_with_last(RowIndex(row));

        // Move last to other column
        other.buffer.resize(other.buffer.len() + other.info.size, MaybeUninit::zeroed());
        let n = self.buffer.len() - self.info.size;
        let m = other.buffer.len() - other.info.size;
        self.buffer[n..].swap_with_slice(&mut other.buffer[m..]);

        // Remove bytes old bytes
        self.buffer.truncate(n);
    }

    // Must change length/overwrite bytes after call
    unsafe fn call_drop(&mut self, RowIndex(row): RowIndex) {
        let bytes = &mut self.buffer[row * self.info.size..][..self.info.size];
        debug_assert_eq!(bytes.len(), self.info.size);
        unsafe {
            (self.info.drop)(&mut self.buffer[row * self.info.size..][..self.info.size]);
        }
    }

    pub fn shrink_to_fit(&mut self, target_chunks: usize) {
        for n in target_chunks..self.no_chunks() {
            // SAFETY: Shrunk after loop
            unsafe { self.call_drop(RowIndex(n)) };
        }
        self.buffer.truncate(target_chunks * self.info.size);
    }

    pub fn swap_drop(&mut self, row: RowIndex) {
        self.swap_with_last(row);
        let n = self.buffer.len() / self.info.size - 1;
        // SAFETY: Immediately shrunk
        unsafe { self.call_drop(RowIndex(n)) };
        self.shrink_to_fit(n);
    }
}

impl Drop for Column {
    fn drop(&mut self) {
        if self.info.size == 0 {
            return;
        }
        for n in (0..self.buffer.len()).step_by(self.info.size) {
            unsafe { (self.info.drop)(&mut self.buffer[n..][..self.info.size]) }
        }
    }
}
