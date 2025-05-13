use std::collections::HashMap;

use derive_more::{Deref, DerefMut};

use crate::entity::Entity;

pub(crate) struct Archetype {
    id: ArchetypeId,
    fields: Vec<FieldId>,
    entities: Vec<Entity>,
    columns: Vec<Column>,
    edges: HashMap<FieldId, ArchetypeEdge>,
}

#[derive(Deref, DerefMut)]
pub(crate) struct ArchetypeId(pub u64);

/// Component or pair
pub(crate) struct FieldId(pub u64);

#[derive(Deref, DerefMut)]
pub(crate) struct Column(pub Vec<u8>);

pub(crate) struct ArchetypeEdge {
    add: *mut Archetype,
    remove: *mut Archetype,
}
