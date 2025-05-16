use std::collections::HashMap;

use derive_more::{Deref, DerefMut};
use parking_lot::RwLock;
use slotmap::new_key_type;

use crate::entity::Entity;

pub(crate) struct Archetype {
    pub fields: Vec<FieldId>,
    pub entities: Vec<Entity>,
    pub columns: Vec<RwLock<Column>>,
    pub edges: HashMap<FieldId, ArchetypeEdge>,
}

new_key_type! { pub(crate) struct ArchetypeId; }

/// Component or pair
#[derive(Clone, Copy, Debug, Eq, Hash, PartialEq)]
pub(crate) struct FieldId(pub u64);

impl From<Entity> for FieldId {
    fn from(entity: Entity) -> Self {
        Self(entity.raw())
    }
}

#[derive(Deref, DerefMut)]
pub(crate) struct Column(pub Vec<u8>);

pub(crate) struct ArchetypeEdge {
    add: ArchetypeId,
    remove: ArchetypeId,
}
