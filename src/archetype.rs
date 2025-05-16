use std::collections::HashMap;

use derive_more::{Deref, DerefMut};
use parking_lot::RwLock;
use slotmap::{KeyData, new_key_type};

use crate::entity::Entity;

#[derive(Default)]
pub(crate) struct Archetype {
    pub fields: Vec<FieldId>,
    pub entities: Vec<Entity>,
    pub columns: Vec<RwLock<Column>>,
    pub edges: HashMap<FieldId, ArchetypeEdge>,
}

new_key_type! { pub(crate) struct ArchetypeId; }

impl ArchetypeId {
    pub(crate) fn null() -> Self {
        Self(KeyData::from_ffi(u64::MAX))
    }

    pub(crate) fn is_empty_archetype(self) -> bool {
        self == Self(KeyData::from_ffi(u64::MAX))
    }
}

/// Component or pair
#[derive(Clone, Copy, Debug, Eq, Hash, PartialEq)]
pub(crate) enum FieldId {
    Component(Entity),
    Pair(u32, u32),
}

impl From<Entity> for FieldId {
    fn from(entity: Entity) -> Self {
        Self::Component(entity)
    }
}

#[derive(Deref, DerefMut)]
pub(crate) struct Column(pub Vec<u8>);

#[derive(Clone, Copy)]
pub(crate) struct ArchetypeEdge {
    add: ArchetypeId,
    remove: ArchetypeId,
}
